// Smoke test: Swift calls the Rust core through the UniFFI bindings.
// CI compiles this with bindings/swift/EncoreCore.swift and bindings/tokens/EncoreTokens.swift
// on Linux. It proves the bindings work; the Apple track proves them on Apple devices.

let greeting = hello(name: "CI")
precondition(greeting == "Hello, CI, from the Encore core.", "unexpected greeting: \(greeting)")
precondition(hello(name: "") == "Hello from the Encore core.")
precondition(appName() == "Encore")
precondition(!coreVersion().isEmpty)
precondition(EncoreTokens.Breakpoints.railWidth == 80)
print("Swift -> Rust: \(greeting) (core \(coreVersion()))")
