// Smoke test: Swift calls the Rust core through the UniFFI bindings.
// CI compiles this with bindings/swift/EncoreCore.swift and bindings/tokens/EncoreTokens.swift
// on Linux, and on macOS for every Apple target in Swift 6 mode: it runs on macOS and in an
// iOS Simulator, and the iOS device build is linked but not run. The Apple track proves the
// bindings on real iPhones, iPads and Macs.

let greeting = hello(name: "CI")
precondition(greeting == "Hello, CI, from the Encore core.", "unexpected greeting: \(greeting)")
precondition(hello(name: "") == "Hello from the Encore core.")
precondition(appName() == "Encore")
precondition(!coreVersion().isEmpty)
precondition(EncoreTokens.Breakpoints.railWidth == 80)
precondition(EncoreTokens.Color.Accents.Iris.Light.primary == "#6146CB")
precondition(EncoreTokens.Typography.Scale.Compact.PageTitle.size == 28)
precondition(EncoreTokens.Recap.TypeGroup.tide == "Inter, heavy") // recap.type, renamed for Swift
print("Swift -> Rust: \(greeting) (core \(coreVersion()))")
