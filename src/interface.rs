use clap::ValueEnum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum InterfaceMode {
    #[default]
    Remote,
    Exttool,
    Simulation,
}

impl InterfaceMode {
    pub const fn default_baud(self) -> u32 {
        match self {
            Self::Remote | Self::Simulation => 19_200,
            Self::Exttool => 38_400,
        }
    }

    pub const fn default_address(self) -> u8 {
        match self {
            Self::Remote | Self::Simulation => 2,
            Self::Exttool => 1,
        }
    }
}
