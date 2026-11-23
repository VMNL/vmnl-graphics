// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Observe tracked joystick events and gamepad state through a window.

use vmnl_graphics::{
    Context, EventKind, GamepadButton, JoystickId, StickAngleConvention, StickConfig, VMNLResult,
    Window,
};

fn print_joystick_snapshot(window: &Window) {
    let joystick = window.input().joystick(JoystickId::Joystick1);
    if joystick.is_pressed(GamepadButton::A) {
        println!(
            "[gamepad] A pressed; left stick={:?}",
            joystick
                .left_stick()
                .map(|stick| (stick.x(), stick.y(), stick.angle()))
        );
    }
    if joystick.is_released(GamepadButton::A) {
        println!("[gamepad] A released");
    }
}

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let mut window = Window::builder()
        .title("VMNL input_joystick")
        .size(800, 460)
        .set_clear_color([20, 24, 32, 255])
        .build(&context)?;

    window.set_joystick_event_delivery(true);
    window.set_joystick_tracking(true)?;
    let left = StickConfig::default().with_radial_dead_zone(0.15)?;
    let right = left.with_angle_convention(StickAngleConvention::Screen2D);
    window.set_left_stick_config(left);
    window.set_right_stick_config(right);
    println!("Connect a controller in slot 1 and press/release gamepad A. A mapping is required for gamepad fields.");

    while window.is_open() {
        for event in window.poll_events() {
            if matches!(
                event.kind(),
                EventKind::JoystickConnected { .. } | EventKind::JoystickDisconnected { .. }
            ) {
                println!("{:?}", event.kind());
            }
        }
        print_joystick_snapshot(&window);
        if window.is_open() {
            window.render().submit()?;
        }
    }

    Ok(())
}
