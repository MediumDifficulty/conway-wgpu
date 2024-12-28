use std::collections::VecDeque;

use gilrs::{ev::{self, AxisOrBtn}, Axis, Gilrs};
use winit::{event::WindowEvent, keyboard::KeyCode};

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

/// `T` is a user declared enum to identify different handlers
pub struct HybridInputState<T> {
    gilrs: Gilrs,
    queue: VecDeque<T>,
    listeners: Vec<(Vec<ListenerSource>, T)>,
    bindings: Vec<(Vec<BindingSource>, T)>
}

pub enum ListenerSource {
    Key(KeyCode),
    Gamepad(AxisOrBtn)
}

pub enum BindingSource {
    Key {
        state: bool,
        key: KeyCode
    },
    GamePadAxis(Axis)
}


impl<T> HybridInputState<T> where T: Copy {
    pub fn handle_winit(&mut self, event: &WindowEvent) -> bool {
        let mut consumed = false;
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                let code = match event.physical_key {
                    winit::keyboard::PhysicalKey::Code(key_code) => key_code,
                    winit::keyboard::PhysicalKey::Unidentified(_) => return false,
                };
        
                let pressed = event.state.is_pressed();
        
                for (sources, _) in &mut self.bindings {
                    for source in sources.iter_mut() {
                        match source {
                            BindingSource::Key { state, key } => if *key == code {
                                *state = pressed;
                                consumed = true;
                            },
                            _ => {},
                        }
                    }
                }
        
                for (sources, ident) in &mut self.listeners {
                    for source in sources.iter_mut() {
                        match source {
                            ListenerSource::Key(key_code) => if *key_code == code {
                                self.queue.push_front(*ident);
                                consumed = true;
                            },
                            _ => {},
                        }
                    }
                }
            }
            // TODO: WindowEvent::MouseInput { device_id, state, button }
            _ => {}
        }   

        consumed
    }

    pub fn next_event(&mut self) -> Option<T> {
        self.queue.pop_back()
    }

    pub fn update_gamepad(&mut self) {
        while let Some(event) = self.gilrs.next_event() {
            match event.event {
                ev::EventType::ButtonPressed(button, code) => todo!(),
                // ev::EventType::ButtonRepeated(button, code) => todo!(),
                ev::EventType::ButtonReleased(button, code) => todo!(),
                ev::EventType::ButtonChanged(button, _, code) => todo!(),
                ev::EventType::AxisChanged(axis, _, code) => todo!(),
                // ev::EventType::Connected => todo!(),
                // ev::EventType::Disconnected => todo!(),
                _ => todo!(),
            }
        }
    }

    pub fn pressed_amount(&self, ident: T) -> f32 {
        todo!()
    }
}