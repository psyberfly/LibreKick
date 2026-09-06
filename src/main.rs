use nih_plug::prelude::nih_export_standalone_with_args;

use librekick::LibreKick;

fn main() {
    let mut args: Vec<String> = std::env::args().collect();

    #[cfg(target_os = "linux")]
    {
        let has_backend_arg = args.iter().any(|arg| arg == "--backend");
        if !has_backend_arg {
            args.push("--backend".to_owned());
            args.push("alsa".to_owned());
        }
    }

    let _ = nih_export_standalone_with_args::<LibreKick, _>(args);
}
