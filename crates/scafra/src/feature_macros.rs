//! Hidden token adapters used by generated code.
#[cfg(feature = "web")]
#[doc(hidden)]
#[macro_export]
macro_rules! __scafra_if_web { ($($tokens:tt)*) => { $($tokens)* }; }
#[cfg(not(feature = "web"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __scafra_if_web {
    ($($tokens:tt)*) => {};
}
#[cfg(all(feature = "actuator", feature = "security"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __scafra_if_integrations { ($($tokens:tt)*) => { $($tokens)* }; }
#[cfg(not(all(feature = "actuator", feature = "security")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __scafra_if_integrations {
    ($($tokens:tt)*) => {};
}
#[cfg(feature = "web")]
#[doc(hidden)]
#[macro_export]
macro_rules! __scafra_require_web { ($($tokens:tt)*) => { $($tokens)* }; }
#[cfg(not(feature = "web"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __scafra_require_web {
    ($($tokens:tt)*) => {
        compile_error!("this Scafra API requires the `web` Cargo feature");
    };
}
