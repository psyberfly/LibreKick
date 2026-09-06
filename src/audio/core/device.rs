pub trait DeviceParams:
    Clone + Copy + Send + Sync + 'static
{
}

pub trait DeviceEvent:
    Clone + Copy + Send + Sync + 'static
{
}

pub trait Device {
    type Params: DeviceParams;
    type Event: DeviceEvent;

    fn update_params(&mut self, params: Self::Params);

    fn process_event(&mut self, event: Self::Event);

    fn next_sample(&mut self) -> f32;
}

pub trait ControlSnapshot: Clone + Send + Sync + 'static {}

pub trait ControlCommand: Clone + Send + Sync + 'static {}

pub trait ControlledDevice {
    type Snapshot: ControlSnapshot;
    type Command: ControlCommand;

    fn apply_snapshot(&mut self, snapshot: &Self::Snapshot, level: f32);
    fn apply_command(&mut self, command: &Self::Command);
}