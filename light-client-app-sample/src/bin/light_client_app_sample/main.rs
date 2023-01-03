//! Main entry point for LightClientAppSample

#![deny(warnings, missing_docs, trivial_casts, unused_qualifications)]
#![forbid(unsafe_code)]

use light_client_app_sample::application::APP;

/// Boot LightClientAppSample
fn main() {
    abscissa_core::boot(&APP);
}
