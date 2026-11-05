# Platforms

Native applications live here. Neither is implemented yet; each arrives with its phase.

```text
platforms/
├── windows/      Qt 6 / C++ desktop application          (Phase 6)
│   ├── CMakeLists.txt
│   ├── include/
│   ├── src/          application shell, view models, FFI client
│   ├── ui/           QML or widget views per screen
│   └── resources/    icons, fonts, translations, theme tokens
└── android/      Kotlin application                       (Phase 7)
    ├── settings.gradle.kts
    ├── build.gradle.kts
    └── app/          Compose UI, JNI bridge, Keystore integration
```

Rules for both platforms:

* No wallet, asset, amount or address logic is reimplemented in C++ or Kotlin. All of it comes from `xopo-core` through the FFI boundary.
* The only shared code duplicated is presentation: design tokens and reusable components, each written idiomatically for its platform.
* Neither application talks to SQLite, the network or a provider directly.
* Screens are built against `docs/UI_DESIGN_SYSTEM.md`; visual quality and state coverage are acceptance criteria.
