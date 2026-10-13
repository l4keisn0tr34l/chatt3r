# local rustyline-async patch

vendored from the Unlicensed crates.io `rustyline-async` 0.4.9 source
(https://github.com/zyansheep/rustyline-async). only its existing runtime
dependencies are used.

original crates.io archive checksum:
`b76450411764ae23570aa50bae870dac68db933268df2ab3a577d641299d93a2`.

chatt3r changes enable/restore bracketed paste, insert a paste as one draft
without submitting it, and render multiline whitespace with a bounded draft
viewport. pasted terminal controls are escaped for display, not executed.
Enter submits the complete draft; existing history and async output are retained.
