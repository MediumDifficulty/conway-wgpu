use winit::keyboard::KeyCode;

#[derive(Clone)]
pub struct KeyboardInputState {
    bindings: Vec<Vec<(bool, KeyCode)>>
}

impl KeyboardInputState {
    pub fn new(bindings: &[&[KeyCode]]) -> Self {
        Self {
            bindings: bindings.iter()
                .map(|&handler| handler.iter()
                    .map(|&key| (false, key))
                    .collect())
                .collect()
        }
    }

    pub fn handle(&mut self, event: &winit::event::KeyEvent) -> bool {
        let code = match event.physical_key {
            winit::keyboard::PhysicalKey::Code(key_code) => key_code,
            winit::keyboard::PhysicalKey::Unidentified(_) => return false,
        };

        let pressed = event.state.is_pressed();

        let mut consumed = false;
        for handler in &mut self.bindings {
            for state in handler.iter_mut()
                .filter(|key| key.1 == code) {
                state.0 = pressed;
                consumed = true
            }
        }
        consumed
    }

    pub fn is_pressed(&self, idx: usize) -> bool {
        self.bindings[idx].iter().any(|key| key.0)
    }
}