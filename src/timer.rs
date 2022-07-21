#[derive(Debug, Default, Clone, Copy)]
pub struct Timer {
    div: u8,
    tima: u8,
    tma: u8,
    tac: u8,
    pub int_timer_flag: bool,
    int_timer_enable: bool,
    clock_frequency_bit: u16,
    current_cycle: u16,
    prev_and_result: bool,
    after_overflow_cycle: u8,
    is_overflowing: bool,
    pub is_stop: bool
}

    

impl Timer {
    pub fn tick(&mut self) {
        if self.is_stop {
            return;
        }

        self.current_cycle = self.current_cycle.wrapping_add(1);
        self.div = ((self.current_cycle >> 8) & 0xFF) as u8;

        let counter_bit = (self.current_cycle & self.clock_frequency_bit) == self.clock_frequency_bit;
        let and_result = counter_bit & self.int_timer_enable;

        if self.prev_and_result && !and_result {
            let (new_tima, is_overflow) = self.tima.overflowing_add(1);
            self.tima = new_tima;
            self.is_overflowing = is_overflow;
        }

        if self.is_overflowing {
            self.after_overflow_cycle = self.after_overflow_cycle.wrapping_add(1);
            if self.after_overflow_cycle == 4 {
                self.tima = self.tma;
                self.after_overflow_cycle = 0;
                self.int_timer_flag = true;
                self.is_overflowing = false;
            }
        }

        self.prev_and_result = and_result;
    }

    pub fn read_div(&self) -> u8 {
        return self.div;
    }

    pub fn read_tima(&self) -> u8 {
        return self.tima;
    }

    pub fn read_tma(&self) -> u8 {
        return self.tma;
    }

    pub fn read_tac(&self) -> u8 {
        return self.tac;
    }

    pub fn write_div(&mut self, _: u8) {
        // Writing any value to this register resets it to $00.
        self.div = 0;
    }

    pub fn write_tima(&mut self, data: u8) {
        self.tima = data;
        self.is_overflowing = false;
        self.after_overflow_cycle = 0;
    }

    pub fn write_tma(&mut self, data: u8) {
        self.tma = data;
    }

    pub fn write_tac(&mut self, data: u8) {
        self.tac = data;
        self.int_timer_enable = (data & (1 << 2)) == (1 << 2);
        match data & 0x3 {
            0 => self.clock_frequency_bit = 1 << 9,
            1 => self.clock_frequency_bit = 1 << 3,
            2 => self.clock_frequency_bit = 1 << 5,
            3 | _ => self.clock_frequency_bit = 1 << 7
        }
    }
}
