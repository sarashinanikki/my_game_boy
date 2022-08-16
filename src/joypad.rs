pub enum Button {
    Right,
    Left,
    Up,
    Down,
    A,
    B,
    Select,
    Start
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Joypad {
    right: bool,
    left: bool,
    up: bool,
    down: bool,
    a: bool,
    b: bool,
    select: bool,
    start: bool,
    p15: bool,
    p14: bool,
    pub int_flag: bool
}

impl Joypad {
    pub fn write(&mut self, data: u8) {
        self.p15 = data & (1 << 5) == (1 << 5);
        self.p14 = data & (1 << 4) == (1 << 4);
    }

    pub fn read(&self) -> u8 {
        if self.p15 && self.p14 {
            return 0xFF;
        }
        
        // ボタンの状態を読み込む
        if !self.p15 {
            let mut ret = 0xDF;
            ret &= !((self.start as u8) << 3);
            ret &= !((self.select as u8) << 2);
            ret &= !((self.b as u8) << 1);
            ret &= !((self.a as u8) << 0);

            // println!("Button = {ret:#08b}");
            return ret
        }

        // 十字キーの状態を読み込む
        if !self.p14 {
            let mut ret = 0xEF;
            ret &= !((self.down as u8) << 3);
            ret &= !((self.up as u8) << 2);
            ret &= !((self.left as u8) << 1);
            ret &= !((self.right as u8) << 0);

            // println!("Direction = {ret:#08b}");
            return ret
        }

        return 0xFF
    }

    pub fn press(&mut self, button: Button) {
        self.int_flag = true;
        match button {
            Button::Right => self.right = true,
            Button::Left => self.left = true,
            Button::Up => self.up = true,
            Button::Down => self.down = true,
            Button::A => self.a = true,
            Button::B => self.b = true,
            Button::Select => self.select = true,
            Button::Start => self.start = true
        }
    }

    pub fn release(&mut self, button: Button) {
        match button {
            Button::Right => self.right = false,
            Button::Left => self.left = false,
            Button::Up => self.up = false,
            Button::Down => self.down = false,
            Button::A => self.a = false,
            Button::B => self.b = false,
            Button::Select => self.select = false,
            Button::Start => self.start = false
        }
    }
}
