use std::collections::VecDeque;

use gilrs::{Axis, Gilrs};
use winit::{event::{KeyEvent, MouseButton, WindowEvent}, keyboard::{KeyCode, PhysicalKey}};

/// `T` is a user declared enum to identify different handlers
pub struct HybridInputState<T> {
    gilrs: Gilrs,
    queue: VecDeque<T>,
    listeners: Vec<(Vec<InputSource>, T)>,
    bindings: Vec<(Vec<InputSource>, T)>
}

#[derive(Clone)]
pub enum InputSource {
    Key {
        state: bool,
        key: KeyCode
    },
    GamepadAxis {
        axis: Axis,
        mapping: fn(f32) -> f32
    },
    GamepadButton(gilrs::Button),
    Mouse {
        state: bool,
        button: MouseButton
    }
}

impl InputSource {
    pub fn key(key: KeyCode) -> Self {
        Self::Key { state: false, key }
    }

    pub fn axis(axis: Axis, mapping: fn(f32) -> f32) -> Self {
        Self::GamepadAxis { axis, mapping }
    }

    /// Updates internal state along with returning if the event was processed
    fn handle_winit(&mut self, event: &WindowEvent) -> bool {
        match (event, self) {
            (
                WindowEvent::KeyboardInput { event: KeyEvent { physical_key: PhysicalKey::Code(code), state: element_state, .. }, .. },
                InputSource::Key { state, key }
            ) if code == key => {
                *state = element_state.is_pressed();
                true
            },
            (
                WindowEvent::MouseInput { button: mouse_button, state: element_state, .. },
                InputSource::Mouse { state, button }
            ) if mouse_button == button => {
                *state = element_state.is_pressed();
                true
            },
            _ => false
        }
    }

    fn matches_gamepad(&self, event: &gilrs::Event) -> bool {
        matches!((event.event, self), (
            gilrs::EventType::ButtonPressed(gamepad_button, _),
            InputSource::GamepadButton(button)
        ) if gamepad_button == *button)
    }

    fn pressed_amount(&self, gilrs: &Gilrs) -> f32 {
        match self {
            InputSource::Key { state, .. } => *state as u32 as f32,
            InputSource::Mouse { state, .. } => *state as u32 as f32,
            InputSource::GamepadAxis { axis, mapping } => gilrs.gamepads().next()
                .and_then(|(_, gamepad)| gamepad.axis_data(*axis).cloned())
                .map(|data| mapping(data.value()))
                .unwrap_or_default(),
            InputSource::GamepadButton(button) => gilrs.gamepads().next()
                .and_then(|(_, gamepad)| gamepad.button_data(*button).cloned())
                .map(|data| data.is_pressed() as u32 as f32)
                .unwrap_or_default(),
        }
    }
}

impl<T> HybridInputState<T> where T: Copy + Eq {
    pub fn new(bindings: &[(&[InputSource], T)], listeners: &[(&[InputSource], T)]) -> Self {
        Self {
            gilrs: Gilrs::new().unwrap(),
            queue: VecDeque::new(),
            listeners: listeners.iter().map(|(a, b)| (a.to_vec(), *b)).collect(),
            bindings: bindings.iter().map(|(a, b)| (a.to_vec(), *b)).collect(),
        }
    }

    pub fn handle_winit(&mut self, event: &WindowEvent) -> bool {
        let mut consumed = false;

        for (sources, _) in &mut self.bindings {
            for source in sources.iter_mut() {
                consumed |= source.handle_winit(event);
            }
        }

        for (sources, ident) in &mut self.listeners {
            for source in sources.iter_mut() {
                if source.handle_winit(event) {
                    self.queue.push_front(*ident);
                    consumed = true;
                }
            }
        }

        consumed
    }

    pub fn next_event(&mut self) -> Option<T> {
        self.queue.pop_back()
    }

    pub fn update_gamepad(&mut self) {
        while let Some(event) = self.gilrs.next_event() {
            for (sources, ident) in &mut self.bindings {
                for source in sources.iter_mut() {
                    if source.matches_gamepad(&event) {
                        self.queue.push_front(*ident);
                    }
                }
            }
        }
    }

    pub fn pressed_amount(&self, ident: T) -> f32 {
        self.bindings.iter()
            .filter(|(_, id)| *id == ident) // TODO: Change this to a find as there *should* only be one ident present
            .flat_map(|(sources, _)| sources.iter())
            .map(|source| source.pressed_amount(&self.gilrs))
            .max_by(|a, b| a.total_cmp(b))
            .unwrap_or_default()
    }
}
