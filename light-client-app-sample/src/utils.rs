//! Util functions/macors

/// Print info message with current time formatted.
#[macro_export]
macro_rules! info_with_time {
    ($fmt:expr, $($arg:tt)+) => {
        abscissa_core::status_info!("Info",
            format!("{}\t{}",
                chrono::Local::now().naive_local().format("%m-%d %H:%M:%S").to_string(),
                format!($fmt, $($arg)+)
            )
        );
    };
}
