//! Android only: `uk.gcartee.encore.core.EncoreTls.initialize(context)`, which sets up the
//! operating system's certificate verifier (docs/requests/core-0001-android-tls-init.md).
//! The Kotlin side is `core/bindings/kotlin-android/uk/gcartee/encore/core/EncoreTls.kt`.

use jni::EnvUnowned;
use jni::objects::{JClass, JObject};

/// Native side of `EncoreTls.initialize`. Any error or panic becomes a Java
/// RuntimeException instead of crossing into the JVM.
#[unsafe(no_mangle)]
pub extern "system" fn Java_uk_gcartee_encore_core_EncoreTls_initialize<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    context: JObject<'caller>,
) {
    unowned_env
        .with_env(|env| -> Result<(), jni::errors::Error> {
            encore_sources::http::android::init(env, context)
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}
