use core::arch::asm;

pub struct Pci {
    pub devices: [Device; Self::CAPACITY],
    pub num_device: usize
}

impl Pci {
    const CONFIG_ADDRESS: u16 = 0x0cf8;
    const CONFIG_DATA: u16 = 0x0cfc;

    const CAPACITY: usize = 256;
    const MAX_DEVICE: u8 = 32;
    const MAX_FUNCTION: u8 = 8;

    pub fn scan() -> (usize, [Device; Self::CAPACITY]) {
        let mut _self = Self { 
            devices: [Device::new(0, 0, 0); Self::CAPACITY], 
            num_device: 0 
        };

        if Self::is_single_function(0, 0, 0) {
            _self.scan_bus(0);
        } else {
            for function in 1..Self::MAX_FUNCTION {
                if Self::vendor_id(0, 0, function) == 0xffff {
                    continue;
                }
                _self.scan_bus(function);
            }
        }

        (_self.num_device, _self.devices)
    }

    fn scan_bus(&mut self, bus: u8) {
        for device in 0..Self::MAX_DEVICE {
            if Self::vendor_id(bus, device, 0) == 0xffff {
                continue;
            }
            self.scan_device(bus, device);
        }
    }

    fn scan_device(&mut self, bus: u8, device: u8) {
        if Self::is_single_function(bus, device, 0) {
            self.scan_function(bus, device, 0);
        } else {
            for function in 1..Self::MAX_FUNCTION {
                if Self::vendor_id(bus, device, function) == 0xffff {
                    continue;
                }
                self.scan_function(bus, device, function);
            }
        }
    }

    fn scan_function(&mut self, bus: u8, device: u8, function: u8) {
        self.add_device(bus, device, function);
        
        let class_code = Self::class_code(bus, device, function);
        let base = (class_code >> 24) as u8;
        let sub = (class_code >> 16) as u8;

        if base == 0x06 && sub == 0x04 {
            let bus_numbers = Self::bus_numbers(bus, device, function);
            let secondary_bus = (bus_numbers >> 8) as u8;
            self.scan_bus(secondary_bus);
        }
    }

    fn add_device(&mut self, bus: u8, device: u8, function: u8) {
        if self.num_device == self.devices.len() {
            return;
        }

        self.devices[self.num_device] = Device::new(bus, device, function);
        self.num_device += 1;
    }

    fn address(bus: u8, device: u8, function: u8, reg_addr: u8) -> u32 {
        (1 << 31)
            | ((bus as u32) << 16)
            | ((device as u32) << 11)
            | ((function as u32) << 8)
            | ((reg_addr as u32) & 0xfc)
    }

    fn read(address: u32) -> u32 {
        let out;
        unsafe {
            asm!("out dx, eax", in("dx") Self::CONFIG_ADDRESS, in("eax") address);
            asm!("in eax, dx", out("eax") out, in("dx") Self::CONFIG_DATA);
        }
        out
    }

    fn vendor_id(bus: u8, device: u8, function: u8) -> u16 {
        let address = Self::address(bus, device, function, 0x00);
        (Self::read(address) & 0xffff) as u16
    }

    fn class_code(bus: u8, device: u8, function: u8) -> u32 {
        let address = Self::address(bus, device, function, 0x08);
        Self::read(address)
    }

    fn header_type(bus: u8, device: u8, function: u8) -> u8 {
        let address = Self::address(bus, device, function, 0x0c);
        (Self::read(address) >> 16) as u8
    }

    fn bus_numbers(bus: u8, device: u8, function: u8) -> u32 {
        let address = Self::address(bus, device, function, 0x18);
        Self::read(address)
    }

    fn is_single_function(bus: u8, device: u8, function: u8) -> bool {
        Self::header_type(bus, device, function) & 0x80 == 0
    }
}

#[derive(Copy, Clone)]
pub struct Device {
    pub bus: u8,
    pub device: u8,
    pub function: u8
}

impl Device {
    fn new(bus: u8, device: u8, function: u8) -> Self {
        Self { bus, device, function }
    }

    pub fn vendor_id(&self) -> u16 {
        let address = Pci::address(self.bus, self.device, self.function, 0x00);
        (Pci::read(address) & 0xffff) as u16
    }

    pub fn class_code(&self) -> u32 {
        let address = Pci::address(self.bus, self.device, self.function, 0x08);
        Pci::read(address)
    }

    pub fn header_type(&self) -> u8 {
        let address = Pci::address(self.bus, self.device, self.function, 0x0c);
        (Pci::read(address) >> 16) as u8
    }
}