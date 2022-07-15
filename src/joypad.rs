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
    p14: bool
}

impl Joypad {
    pub fn write(&mut self, data: u8) {
        self.p15 = data & (1 << 5) == (1 << 5);
        self.p14 = data & (1 << 4) == (1 << 4);
    }

    pub fn read(&self) -> u8 {
        if self.p15 && self.p14 {
            return 0x3F;
        }
        
        // ボタンの状態を読み込む
        if !self.p15 {
            let mut ret = 0x1F;
            ret = ret & !((self.start as u8) << 3);
            ret = ret & !((self.select as u8) << 2);
            ret = ret & !((self.b as u8) << 1);
            ret = ret & !((self.a as u8) << 0);

            return ret
        }

        // 十字キーの状態を読み込む
        if !self.p14 {
            let mut ret = 0x2F;
            ret = ret & !((self.down as u8) << 3);
            ret = ret & !((self.up as u8) << 2);
            ret = ret & !((self.left as u8) << 1);
            ret = ret & !((self.right as u8) << 0);

            return ret
        }

        return 0x3F
    }

    pub fn press(&mut self, button: Button) {
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
