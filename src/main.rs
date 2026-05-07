use nih_plug::prelude::nih_export_standalone;

use librekick::LibreKick;

fn main() {
    let _ = nih_export_standalone::<LibreKick>();
}
