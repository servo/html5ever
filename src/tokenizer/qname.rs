
enum QNameState {
    BeforeName,
    InName,
    AfterColon,
}

pub struct QNameTokenizer<'a> {
    state : QNameState,
    slice: &'a [u8],
    valid_index: Option<u32>,
    curr_ind: usize,
}

impl<'a> QNameTokenizer<'a> {
    pub fn new(tag: &[u8]) -> QNameTokenizer {
        QNameTokenizer {
            state: QNameState::BeforeName,
            slice: tag,
            valid_index: None,
            curr_ind: 0,
        }
    }

    pub fn run(&mut self) -> Option<u32> {
        if self.slice.len() > 0 {
            while self.step() {
            }
        }
        self.valid_index
    }

    fn incr(&mut self) -> bool {
        if self.curr_ind + 1 < self.slice.len() {
            self.curr_ind += 1;
            return true;
        }
        false
    }

    fn step(&mut self) -> bool {
        match self.state {
            QNameState::BeforeName => self.do_before_name(),
            QNameState::InName => self.do_in_name(),
            QNameState::AfterColon   => self.do_after_colon(),
        }
    }

    fn do_before_name(&mut self) -> bool {
        if self.slice[self.curr_ind] == b':' {
            false
        } else {
            self.state = QNameState::InName;
            self.incr()
        }
    }

    fn do_in_name(&mut self) -> bool {
        if self.slice[self.curr_ind] == b':' && self.curr_ind +1 < self.slice.len() {
            self.valid_index = Some(self.curr_ind as u32);
            self.state = QNameState::AfterColon;
        }
        self.incr()
    }

    fn do_after_colon(&mut self) -> bool {
        if self.slice[self.curr_ind] == b':' {
            self.valid_index = None;
            return false;
        }
        self.incr()
    }

}

