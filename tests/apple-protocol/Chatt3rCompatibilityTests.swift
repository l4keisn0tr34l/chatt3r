// Synthetic fixtures only. Inject into the pinned stock BitFoundationTests target.
import BitFoundation
import CryptoKit
import Foundation
import XCTest

final class Chatt3rCompatibilityTests: XCTestCase {
    func testPublicTextCompatibility() throws {
        let path = try XCTUnwrap(
            ProcessInfo.processInfo.environment["CHATT3R_VECTORS"],
            "Set CHATT3R_VECTORS to public-text-long.json"
        )
        let vectors = try JSONDecoder().decode(
            Vectors.self, from: Data(contentsOf: URL(fileURLWithPath: path))
        )
        XCTAssertEqual(vectors.cases.count, 17, "Exercise every current synthetic case")
        XCTAssertEqual(Set(vectors.cases.map(\.label)).count, vectors.cases.count)
        let publicKey = try Curve25519.Signing.PublicKey(
            rawRepresentation: bytes(hex: vectors.public_key_hex)
        )

        for vector in vectors.cases {
            do {
                let wire = try bytes(hex: vector.wire_hex)
                let expectedPayload = try bytes(hex: vector.decoded_hex)
                let expectedSigning = try bytes(hex: vector.signing_hex)
                let packet = try XCTUnwrap(BitchatPacket.from(wire), "\(vector.label): stock decode")
                let actualSigning = try XCTUnwrap(
                    packet.toBinaryDataForSigning(), "\(vector.label): stock signing encoding"
                )
                let actualWire = try XCTUnwrap(
                    packet.toBinaryData(padding: false), "\(vector.label): stock wire encoding"
                )
                let signature = try XCTUnwrap(packet.signature, "\(vector.label): signature")
                let decodedMatches = packet.payload == expectedPayload
                let wireMatches = actualWire == wire
                let signingMatches = actualSigning == expectedSigning
                let signatureValid = publicKey.isValidSignature(signature, for: actualSigning)

                if !signingMatches {
                    // The bytes are public synthetic test material, never a device capture.
                    let message = "case=\(vector.label) "
                        + "expected_sha256=\(hex(SHA256.hash(data: expectedSigning))) "
                        + "actual_sha256=\(hex(SHA256.hash(data: actualSigning))) "
                        + "actual_signing_hex=\(hex(actualSigning))"
                    print("::error title=chatt3r canonical mismatch::\(annotation(message))")
                }
                if !wireMatches {
                    let message = "case=\(vector.label) "
                        + "expected_sha256=\(hex(SHA256.hash(data: wire))) "
                        + "actual_sha256=\(hex(SHA256.hash(data: actualWire))) "
                        + "actual_wire_hex=\(hex(actualWire))"
                    print("::error title=chatt3r wire mismatch::\(annotation(message))")
                }

                print("[chatt3r-compat] case=\(vector.label) decoded=\(decodedMatches) "
                    + "wire=\(wireMatches) canonical=\(signingMatches) signature=\(signatureValid)")
                XCTAssertTrue(decodedMatches, "\(vector.label): decoded payload differs")
                XCTAssertTrue(wireMatches, "\(vector.label): re-encoded wire differs")
                XCTAssertTrue(signingMatches, "\(vector.label): canonical signing bytes differ")
                XCTAssertTrue(signatureValid, "\(vector.label): stock canonical signature invalid")
            } catch {
                print("[chatt3r-compat] case=\(vector.label) error=\(error)")
                let message = annotation("case=\(vector.label) error=\(error)")
                print("::error title=chatt3r stock codec failure::\(message)")
                XCTFail("\(vector.label): \(error)")
            }
        }
    }

    private struct Vectors: Decodable {
        let public_key_hex: String
        let cases: [Vector]
    }

    private struct Vector: Decodable {
        let label: String
        let decoded_hex: String
        let signing_hex: String
        let wire_hex: String
    }

    private func bytes(hex: String) throws -> Data {
        let encoded = Array(hex.utf8)
        guard encoded.count.isMultiple(of: 2) else {
            throw NSError(domain: "Chatt3rCompatibility", code: 1,
                          userInfo: [NSLocalizedDescriptionKey: "odd hex length"])
        }
        var result = Data()
        for index in stride(from: 0, to: encoded.count, by: 2) {
            let pair = String(decoding: encoded[index..<(index + 2)], as: UTF8.self)
            guard let byte = UInt8(pair, radix: 16) else {
                throw NSError(domain: "Chatt3rCompatibility", code: 2,
                              userInfo: [NSLocalizedDescriptionKey: "invalid hex byte"])
            }
            result.append(byte)
        }
        return result
    }

    private func hex<Bytes: Sequence>(_ bytes: Bytes) -> String where Bytes.Element == UInt8 {
        bytes.map { String(format: "%02x", $0) }.joined()
    }

    private func annotation(_ message: String) -> String {
        message.replacingOccurrences(of: "%", with: "%25")
            .replacingOccurrences(of: "\r", with: "%0D")
            .replacingOccurrences(of: "\n", with: "%0A")
    }
}
