use core::{arch::asm, num};

pub struct Pci {
    pub devices: [Device; Self::CAPACITY],
    pub num_device: usize,
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
            num_device: 0,
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

    fn write(address: u32, value: u32) {
        unsafe {
            asm!("out dx, eax", in("dx") Self::CONFIG_ADDRESS, in("eax") address);
            asm!("out dx, eax", in("dx") Self::CONFIG_DATA, in("eax") value);
        }
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
    pub function: u8,
}

impl Device {
    const MSI: u8 = 0x05;
    const MSIX: u8 = 0x11;

    fn new(bus: u8, device: u8, function: u8) -> Self {
        Self {
            bus,
            device,
            function,
        }
    }

    pub fn read(&self, addr: u8) -> u32 {
        let address = Pci::address(self.bus, self.device, self.function, addr);
        Pci::read(address)
    }

    pub fn write(&self, addr: u8, value: u32) {
        let address = Pci::address(self.bus, self.device, self.function, addr);
        Pci::write(address, value);
    }

    pub fn vendor_id(&self) -> u16 {
        (self.read(0x00) & 0xffff) as u16
    }

    pub fn class_code(&self) -> ClassCode {
        let reg = self.read(0x08);
        ClassCode::new((reg >> 24) as u8, (reg >> 16) as u8, (reg >> 8) as u8)
    }

    pub fn header_type(&self) -> u8 {
        (self.read(0x0c) >> 16) as u8
    }

    pub fn bar(&self, bar_index: u8) -> Result<usize, &str> {
        if bar_index >= 6 {
            return Err("Index out of bounds");
        }

        let bar_addr = 0x10 + 4 * bar_index;
        let bar = self.read(bar_addr) as usize;

        if bar & 0x4 == 0 {
            return Ok(bar);
        }

        if bar_index >= 5 {
            return Err("Index out of bounds");
        }

        let bar_upper = self.read(bar_addr + 4) as usize;

        Ok(bar | bar_upper << 32)
    }

    fn capability_pointer(&self) -> u8 {
        self.read(0x34) as u8
    }

    fn capability_structure(&self, addr: u8) -> Capability {
        let cap = self.read(addr);
        Capability {
            cap_id: cap as u8,
            next_ptr: (cap >> 8) as u8,
        }
    }

    fn msi_capability_structure(&self, addr: u8) -> MSICapability {
        let mut msi_cap = MSICapability::default();

        msi_cap.header = self.read(addr);
        msi_cap.msg_addr = self.read(addr + 4);

        let mut msg_data_addr = addr + 8;
        if msi_cap.addr_64_capable() {
            msi_cap.msg_upper_addr = self.read(addr + 8);
            msg_data_addr = addr + 12;
        }

        msi_cap.msg_data = self.read(msg_data_addr);

        if msi_cap.per_vector_mask_capable() {
            msi_cap.mask_bits = self.read(msg_data_addr + 4);
            msi_cap.pending_bits = self.read(msg_data_addr + 8);
        }

        msi_cap
    }

    fn set_msi_capability_structure(&self, addr: u8, msi_cap: MSICapability) {
        self.write(addr, msi_cap.header);
        self.write(addr + 4, msi_cap.msg_addr);

        let mut msg_data_addr = addr + 8;
        if msi_cap.addr_64_capable() {
            self.write(addr + 8, msi_cap.msg_upper_addr);
            msg_data_addr = addr + 12;
        }

        self.write(msg_data_addr, msi_cap.msg_data);

        if msi_cap.per_vector_mask_capable() {
            self.write(msg_data_addr + 4, msi_cap.mask_bits);
            self.write(msg_data_addr + 8, msi_cap.pending_bits);
        }
    }

    pub fn configure_msi(
        &self,
        msg_addr: u32,
        msg_data: u32,
        num_vector_exponent: u8,
    ) -> Result<(), &str> {
        let mut msi_cap_addr = 0u8;
        let mut msix_cap_addr = 0u8;

        let mut cap_addr = self.capability_pointer();
        while cap_addr != 0 {
            let cap = self.capability_structure(cap_addr);
            if cap.cap_id == Self::MSI {
                msi_cap_addr = cap_addr;
            } else if cap.cap_id == Self::MSIX {
                msix_cap_addr = cap_addr;
            }
            cap_addr = cap.next_ptr;
        }

        if msi_cap_addr != 0 {
            return self._configure_msi(msi_cap_addr, msg_addr, msg_data, num_vector_exponent);
        } else if msix_cap_addr != 0 {
            return self._configure_msix(msix_cap_addr, msg_addr, msg_data, num_vector_exponent);
        }

        Err("No MSI Capability")
    }

    fn _configure_msi(
        &self,
        cap_addr: u8,
        msg_addr: u32,
        msg_data: u32,
        num_vector_exponent: u8,
    ) -> Result<(), &str> {
        let mut msi_cap = self.msi_capability_structure(cap_addr);

        msi_cap.set_multi_msg_enable(if msi_cap.multi_msg_capable() <= num_vector_exponent {
            msi_cap.multi_msg_capable()
        } else {
            num_vector_exponent
        });

        msi_cap.set_msi_enable();

        msi_cap.msg_addr = msg_addr;
        msi_cap.msg_data = msg_data;

        self.set_msi_capability_structure(cap_addr, msi_cap);

        Ok(())
    }

    fn _configure_msix(
        &self,
        _cap_addr: u8,
        _msg_addr: u32,
        _msg_data: u32,
        _num_vector_exponent: u8,
    ) -> Result<(), &str> {
        Ok(())
    }
}

impl core::fmt::Display for Device {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:02x}.{:02x}.{:02x}",
            self.bus, self.device, self.function
        )
    }
}

#[derive(PartialEq)]
pub struct ClassCode {
    base: u8,
    sub: u8,
    interface: u8,
}

impl ClassCode {
    pub fn new(base: u8, sub: u8, interface: u8) -> Self {
        Self {
            base,
            sub,
            interface,
        }
    }
}

impl core::fmt::Display for ClassCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:02x}{:02x}{:02x}", self.base, self.sub, self.interface)
    }
}

struct Capability {
    cap_id: u8,
    next_ptr: u8,
}

#[derive(Default)]
struct MSICapability {
    header: u32,
    // struct {
    //     uint32_t capability_id : 8;
    //     uint32_t next_pointer : 8;
    //     uint32_t msi_enable : 1;
    //     uint32_t multi_msg_capable : 3;
    //     uint32_t multi_msg_enable : 3;
    //     uint32_t addr_64_capable : 1;
    //     uint32_t per_vector_mask_capable : 1;
    //     uint32_t : 7;
    // } bits;
    msg_addr: u32,
    msg_upper_addr: u32,
    msg_data: u32,
    mask_bits: u32,
    pending_bits: u32,
}

impl MSICapability {
    fn capability_id(&self) -> u8 {
        self.header as u8
    }
    fn next_pointer(&self) -> u8 {
        (self.header >> 8) as u8
    }
    fn msi_enable(&self) -> bool {
        (self.header >> 16) & 0b1 != 0
    }
    fn multi_msg_capable(&self) -> u8 {
        (self.header >> 17) as u8 & 0b111
    }
    fn multi_msg_enable(&self) -> u8 {
        (self.header >> 20) as u8 & 0b111
    }
    fn addr_64_capable(&self) -> bool {
        (self.header >> 23) & 0b1 != 0
    }
    fn per_vector_mask_capable(&self) -> bool {
        (self.header >> 24) & 0b1 != 0
    }
    fn set_msi_enable(&mut self) {
        self.header |= 0b1u32 << 16;
    }
    fn clear_msi_enable(&mut self) {
        self.header &= !(0b1u32 << 16);
    }
    fn set_multi_msg_enable(&mut self, val: u8) {
        self.header |= (val as u32 & 0b111) << 20;
    }
}
