//! Native Windows peripheral role. This is separate from btleplug's proven
//! central path; it advertises one GATT service and receives writes from one
//! subscribed Linux central. Hardware interoperability still needs a PC test.
use super::{Result, CHARACTERISTIC, SERVICE};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc as blocking, Arc, Mutex,
};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration, Instant};
use windows::core::GUID;
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristicProperties as Props, GattCommunicationStatus, GattLocalCharacteristic,
    GattLocalCharacteristicParameters, GattServiceProvider,
    GattServiceProviderAdvertisementStatus as AdStatus, GattServiceProviderAdvertisingParameters,
    GattSubscribedClient, GattWriteOption, GattWriteRequestedEventArgs,
};
use windows::Devices::Bluetooth::{BluetoothAdapter, BluetoothError};
use windows::Foundation::{Deferral, TypedEventHandler};
use windows::Storage::Streams::{DataReader, DataWriter, IBuffer};
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};

fn bytes(buffer: &IBuffer) -> windows::core::Result<Vec<u8>> {
    let reader = DataReader::FromBuffer(buffer)?;
    let mut value = vec![0; buffer.Length()? as usize];
    reader.ReadBytes(&mut value)?;
    Ok(value)
}

fn buffer(value: &[u8]) -> windows::core::Result<IBuffer> {
    let writer = DataWriter::new()?;
    writer.WriteBytes(value)?;
    writer.DetachBuffer()
}

fn sole_client(
    characteristic: &GattLocalCharacteristic,
) -> windows::core::Result<Option<GattSubscribedClient>> {
    let clients = characteristic.SubscribedClients()?;
    if clients.Size()? == 1 {
        Ok(Some(clients.GetAt(0)?))
    } else {
        Ok(None)
    }
}

fn device_id(client: &GattSubscribedClient) -> windows::core::Result<String> {
    Ok(client.Session()?.DeviceId()?.Id()?.to_string())
}

// A single worker preserves write/fragment order. Never block the WinRT event
// callback on an async request; keep the event alive with its deferral.
fn process_write(
    args: GattWriteRequestedEventArgs,
    characteristic: &GattLocalCharacteristic,
    incoming: &mpsc::Sender<Vec<u8>>,
    chosen: &Mutex<Option<String>>,
    lost: &AtomicBool,
) -> windows::core::Result<()> {
    let request = args.GetRequestAsync()?.get()?;
    let client = sole_client(characteristic)?;
    let sender = args.Session()?.DeviceId()?.Id()?.to_string();
    let authorized = !lost.load(Ordering::SeqCst)
        && client
            .as_ref()
            .is_some_and(|c| device_id(c).is_ok_and(|id| id == sender))
        && chosen
            .lock()
            .unwrap()
            .as_ref()
            .is_none_or(|id| id == &sender);
    let value = request.Value()?;
    let length = value.Length()?;
    if !authorized || request.Offset()? != 0 || !(1..=512).contains(&length) {
        if request.Option()? == GattWriteOption::WriteWithResponse {
            request.RespondWithProtocolError(0x11)?; // ATT insufficient resources
        }
        return Ok(());
    }
    let frame = bytes(&value)?;
    if incoming.try_send(frame).is_err() {
        if request.Option()? == GattWriteOption::WriteWithResponse {
            request.RespondWithProtocolError(0x11)?;
        }
    } else if request.Option()? == GattWriteOption::WriteWithResponse {
        request.Respond()?;
    }
    Ok(())
}

pub struct GattHost {
    provider: GattServiceProvider,
    characteristic: GattLocalCharacteristic,
    client: Mutex<Option<GattSubscribedClient>>,
    chosen: Arc<Mutex<Option<String>>>,
    lost: Arc<AtomicBool>,
    write_token: i64,
    subscription_token: i64,
}

impl GattHost {
    pub async fn start() -> Result<(Arc<Self>, mpsc::Receiver<Vec<u8>>)> {
        let adapter = BluetoothAdapter::GetDefaultAsync()?.await?;
        if !adapter.IsPeripheralRoleSupported()? {
            return Err(
                "windows adapter reports no BLE peripheral role; try a supported USB adapter"
                    .into(),
            );
        }
        let result = GattServiceProvider::CreateAsync(GUID::from_u128(SERVICE.as_u128()))?.await?;
        if result.Error()? != BluetoothError::Success {
            return Err(format!(
                "windows GATT service creation failed: {:?}",
                result.Error()?
            )
            .into());
        }
        let provider = result.ServiceProvider()?;
        let params = GattLocalCharacteristicParameters::new()?;
        params.SetCharacteristicProperties(Props::Write | Props::Notify)?;
        let char_result = provider
            .Service()?
            .CreateCharacteristicAsync(GUID::from_u128(CHARACTERISTIC.as_u128()), &params)?
            .await?;
        if char_result.Error()? != BluetoothError::Success {
            return Err(format!(
                "windows GATT characteristic creation failed: {:?}",
                char_result.Error()?
            )
            .into());
        }
        let characteristic = char_result.Characteristic()?;
        let (incoming, rx) = mpsc::channel(64);
        let (worker, tasks) = blocking::sync_channel::<(GattWriteRequestedEventArgs, Deferral)>(64);
        let chosen = Arc::new(Mutex::new(None));
        let lost = Arc::new(AtomicBool::new(false));
        let worker_characteristic = characteristic.clone();
        let worker_chosen = Arc::clone(&chosen);
        let worker_lost = Arc::clone(&lost);
        std::thread::spawn(move || {
            // This worker has its own COM apartment; WinRT async request
            // objects are not safe to use from an uninitialized OS thread.
            if let Err(error) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
                eprintln!("[host] could not initialize write worker: {error}");
                return;
            }
            for (args, deferral) in tasks {
                if let Err(error) = process_write(
                    args,
                    &worker_characteristic,
                    &incoming,
                    &worker_chosen,
                    &worker_lost,
                ) {
                    eprintln!("[host] GATT write rejected: {error}");
                }
                let _ = deferral.Complete();
            }
            unsafe {
                RoUninitialize();
            }
        });
        let write_token = characteristic.WriteRequested(&TypedEventHandler::<
            GattLocalCharacteristic,
            GattWriteRequestedEventArgs,
        >::new(move |_, args| {
            if let Some(args) = args.as_ref() {
                let deferral = args.GetDeferral()?;
                if let Err(
                    blocking::TrySendError::Full((_, deferral))
                    | blocking::TrySendError::Disconnected((_, deferral)),
                ) = worker.try_send((args.clone(), deferral))
                {
                    let _ = deferral.Complete();
                }
            }
            Ok(())
        }))?;
        let event_chosen = Arc::clone(&chosen);
        let event_lost = Arc::clone(&lost);
        let subscription_token =
            characteristic.SubscribedClientsChanged(&TypedEventHandler::<
                GattLocalCharacteristic,
                windows::core::IInspectable,
            >::new(move |sender, _| {
                if let Some(sender) = sender.as_ref() {
                    let original = event_chosen.lock().unwrap().clone();
                    if let Some(original) = original {
                        let same = sole_client(sender)?.is_some_and(|client| {
                            device_id(&client).is_ok_and(|id| id == original)
                        });
                        if !same {
                            event_lost.store(true, Ordering::SeqCst);
                        }
                    }
                }
                Ok(())
            }))?;
        let settings = GattServiceProviderAdvertisingParameters::new()?;
        settings.SetIsConnectable(true)?;
        settings.SetIsDiscoverable(true)?;
        if let Err(error) = provider.StartAdvertisingWithParameters(&settings) {
            let _ = characteristic.RemoveWriteRequested(write_token);
            let _ = characteristic.RemoveSubscribedClientsChanged(subscription_token);
            return Err(format!("windows GATT advertising rejected: {error}").into());
        }
        let host = Arc::new(Self {
            provider,
            characteristic,
            client: Mutex::new(None),
            chosen,
            lost,
            write_token,
            subscription_token,
        });
        // StartAdvertising can succeed while the radio later aborts the ad.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match host.provider.AdvertisementStatus()? {
                AdStatus::Started => break,
                AdStatus::Aborted
                | AdStatus::Stopped
                | AdStatus::StartedWithoutAllAdvertisementData => {
                    return Err(format!(
                        "windows GATT advertisement not usable: {:?}",
                        host.provider.AdvertisementStatus()?
                    )
                    .into());
                }
                _ if Instant::now() >= deadline => {
                    return Err("windows GATT advertisement did not start in 10s".into())
                }
                _ => sleep(Duration::from_millis(100)).await,
            }
        }
        Ok((host, rx))
    }

    pub async fn wait_for_client(&self) -> Result<()> {
        loop {
            if self.provider.AdvertisementStatus()? != AdStatus::Started {
                return Err("windows GATT advertisement stopped before subscription".into());
            }
            if let Some(client) = sole_client(&self.characteristic)? {
                let id = device_id(&client)?;
                *self.chosen.lock().unwrap() = Some(id);
                *self.client.lock().unwrap() = Some(client);
                return Ok(());
            }
            sleep(Duration::from_millis(100)).await;
        }
    }

    pub fn connected(&self) -> Result<bool> {
        if self.lost.load(Ordering::SeqCst) {
            return Ok(false);
        }
        let Some(original) = self.client.lock().unwrap().clone() else {
            return Ok(false);
        };
        let Some(current) = sole_client(&self.characteristic)? else {
            return Ok(false);
        };
        Ok(device_id(&original)? == device_id(&current)?)
    }

    pub async fn write(&self, frame: &[u8]) -> Result<()> {
        if !self.connected()? {
            return Err("BLE subscription ended; rerun to reconnect".into());
        }
        let client = self
            .client
            .lock()
            .unwrap()
            .clone()
            .ok_or("no subscribed client")?;
        let result = self
            .characteristic
            .NotifyValueForSubscribedClientAsync(&buffer(frame)?, &client)?
            .await?;
        if result.Status()? != GattCommunicationStatus::Success {
            return Err(format!("windows GATT notification failed: {:?}", result.Status()?).into());
        }
        Ok(())
    }
}

impl Drop for GattHost {
    fn drop(&mut self) {
        let _ = self.provider.StopAdvertising();
        let _ = self.characteristic.RemoveWriteRequested(self.write_token);
        let _ = self
            .characteristic
            .RemoveSubscribedClientsChanged(self.subscription_token);
    }
}
