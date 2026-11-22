// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Public cursor position and input-mode API.

use crate::{Cursor, CursorMode, VMNLResult, Window};

impl Window {
    /// Returns the custom or standard cursor currently assigned to this window.
    ///
    /// `None` means that the backend default cursor is used. The returned borrow remains owned by
    /// the window and compares equal to clones of the resource passed to [`set_cursor`](Self::set_cursor).
    ///
    /// # Example
    ///
    /// See the workflow on [`Self::set_cursor`].
    #[inline]
    #[must_use]
    pub fn cursor(&self) -> Option<&Cursor> {
        self.inner.cursor()
    }

    /// Assigns a custom or standard cursor, or restores the backend default with `None`.
    ///
    /// The window retains a shared clone of the resource, so the caller may drop its own clone or
    /// assign the same cursor to other windows. While the mode is [`CursorMode::Hidden`], this
    /// updates the retained client selection without replacing VMNL's active transparent cursor;
    /// the selection is applied when leaving hidden mode. The selected image may additionally
    /// require window focus on some platforms. This call performs no VMNL allocation and preserves
    /// the previous cursor if GLFW rejects a native request.
    ///
    /// # Arguments
    ///
    /// - `cursor` supplies a cursor built using this window's context, or `None` to restore the
    ///   system default.
    ///
    /// # Errors
    ///
    /// Returns the VMNL category produced by this `glfwSetCursor` call.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use vmnl_graphics::{Context, Cursor, CursorMode, StandardCursor, Window};
    ///
    /// # fn main() -> vmnl_graphics::VMNLResult<()> {
    /// let context = Context::new()?;
    /// let mut window = Window::new(&context)?;
    /// let cursor = Cursor::standard(StandardCursor::Crosshair).build(&context)?;
    /// window.set_cursor(Some(&cursor))?;
    /// assert!(window.cursor().is_some());
    /// window.set_sticky_mouse_buttons(true);
    /// window.set_sticky_keys(true);
    /// window.set_lock_key_modifier_reporting(true);
    /// let _options = (window.is_sticky_mouse_buttons_enabled(),
    ///     window.is_sticky_keys_enabled(), window.is_lock_key_modifier_reporting_enabled());
    ///
    /// window.set_cursor_mode(CursorMode::Disabled)?;
    /// window.set_cursor_position(100.0, 60.0)?;
    /// let _position = window.get_cursor_position();
    /// let _cursor_state = (window.get_cursor_mode(), window.is_cursor_hovered());
    /// if context.is_raw_mouse_motion_supported() {
    ///     window.set_raw_mouse_motion(true)?;
    ///     assert!(window.is_raw_mouse_motion_enabled());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn set_cursor(&mut self, cursor: Option<&Cursor>) -> VMNLResult<()> {
        self.inner.set_cursor(cursor)
    }

    /// Returns the cursor position relative to the upper-left corner of the content area.
    ///
    /// X increases to the right and Y increases downward. Coordinates are `f64` screen
    /// coordinates, not framebuffer pixels, and can be fractional or negative. A native cursor
    /// backend may quantize physical positions. In [`CursorMode::Disabled`], the returned virtual
    /// position is unbounded and retains GLFW's `f64` precision.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self::set_cursor`].
    #[inline]
    #[must_use]
    pub fn get_cursor_position(&self) -> (f64, f64) {
        self.inner.get_cursor_position()
    }

    /// Sets the cursor position relative to the upper-left corner of the content area.
    ///
    /// The request has no effect while the window is unfocused. Wayland only supports updating
    /// the virtual position in [`CursorMode::Disabled`]; other modes report the limitation
    /// through the configured GLFW error callback.
    ///
    /// # Arguments
    ///
    /// - `x` supplies the horizontal coordinate in screen coordinates relative to the content area's
    ///   left edge.
    /// - `y` supplies the vertical coordinate in screen coordinates relative to the content area's
    ///   top edge.
    ///
    /// # Errors
    ///
    /// Returns [`VMNLErrorKind::InvalidState`](crate::VMNLErrorKind::InvalidState) when either
    /// coordinate is not finite.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self::set_cursor`].
    #[inline]
    pub fn set_cursor_position(&mut self, x: f64, y: f64) -> VMNLResult<()> {
        self.inner.set_cursor_position(x, y)
    }

    /// Returns whether the cursor is currently over the window content area.
    ///
    /// On platforms that cannot provide the attribute, GLFW reports an error and returns
    /// `false`.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self::set_cursor`].
    #[inline]
    #[must_use]
    pub fn is_cursor_hovered(&self) -> bool {
        self.inner.is_cursor_hovered()
    }

    /// Returns the cursor mode requested through VMNL for this window.
    ///
    /// Hidden mode is represented by a transparent cursor while GLFW remains in normal mode. The
    /// requested mode can differ from effective native behavior while the window is unfocused or
    /// when a backend cannot implement it.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self::set_cursor`].
    #[inline]
    #[must_use]
    pub fn get_cursor_mode(&self) -> CursorMode {
        self.inner.get_cursor_mode()
    }

    /// Sets the cursor visibility and confinement mode.
    ///
    /// Hidden mode lazily creates and reuses one transparent native cursor per window. The cursor
    /// selected through [`set_cursor`](Self::set_cursor) remains inspectable and is restored when
    /// leaving hidden mode. Disabled and captured modes become effective only while the window is
    /// focused. Captured mode is not implemented by GLFW 3.4 on Cocoa.
    ///
    /// # Arguments
    ///
    /// - `mode` selects a cursor visibility or confinement policy from `CursorMode`.
    ///
    /// # Errors
    ///
    /// Returns the VMNL category produced while creating or assigning the transparent cursor, or
    /// while applying the effective GLFW cursor mode. On failure, VMNL keeps the previously
    /// requested mode and attempts to restore its native cursor state.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self::set_cursor`].
    #[inline]
    pub fn set_cursor_mode(&mut self, mode: CursorMode) -> VMNLResult<()> {
        self.inner.set_cursor_mode(mode)
    }

    /// Returns whether GLFW sticky mouse buttons are enabled for this window.
    ///
    /// This mode affects consuming `glfwGetMouseButton` reads. VMNL does not use those reads for
    /// [`MouseState`](crate::MouseState), so querying VMNL snapshots never consumes a sticky
    /// press or changes batch-transition semantics.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self::set_cursor`].
    #[inline]
    #[must_use]
    pub fn is_sticky_mouse_buttons_enabled(&self) -> bool {
        self.inner.is_sticky_mouse_buttons_enabled()
    }

    /// Enables or disables GLFW sticky mouse buttons for this window.
    ///
    /// Enabling the mode latches a native press until the next consuming `glfwGetMouseButton`
    /// read. VMNL's event-derived [`MouseState`](crate::MouseState) remains non-consuming and
    /// continues to report final state plus every transition in the current event batch.
    ///
    /// # Arguments
    ///
    /// - `enabled` controls whether a button press remains observable until consumed.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self::set_cursor`].
    #[inline]
    pub fn set_sticky_mouse_buttons(&mut self, enabled: bool) {
        self.inner.set_sticky_mouse_buttons(enabled);
    }

    /// Returns whether GLFW sticky keys are enabled for this window.
    ///
    /// This mode affects consuming `glfwGetKey` reads. VMNL does not use those reads for
    /// [`KeyboardState`](crate::KeyboardState), so querying VMNL snapshots never consumes a
    /// sticky press or changes batch-transition semantics.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self::set_cursor`].
    #[inline]
    #[must_use]
    pub fn is_sticky_keys_enabled(&self) -> bool {
        self.inner.is_sticky_keys_enabled()
    }

    /// Enables or disables GLFW sticky keys for this window.
    ///
    /// Enabling the mode latches a native press until the next consuming `glfwGetKey` read.
    /// VMNL's event-derived [`KeyboardState`](crate::KeyboardState) remains non-consuming and
    /// continues to report final state plus every transition in the current event batch.
    ///
    /// # Arguments
    ///
    /// - `enabled` controls whether a key press remains observable until consumed.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self::set_cursor`].
    #[inline]
    pub fn set_sticky_keys(&mut self, enabled: bool) {
        self.inner.set_sticky_keys(enabled);
    }

    /// Returns whether lock-key modifier reporting is enabled for this window.
    ///
    /// When enabled, keyboard and mouse-button events can include
    /// [`Modifiers::CAPS_LOCK`](crate::Modifiers::CAPS_LOCK) and
    /// [`Modifiers::NUM_LOCK`](crate::Modifiers::NUM_LOCK).
    ///
    /// # Example
    ///
    /// See the workflow on [`Self::set_cursor`].
    #[inline]
    #[must_use]
    pub fn is_lock_key_modifier_reporting_enabled(&self) -> bool {
        self.inner.is_lock_key_modifier_reporting_enabled()
    }

    /// Enables or disables lock-key modifier reporting for this window.
    ///
    /// Enabling this mode asks GLFW to include Caps Lock and Num Lock state in modifier payloads
    /// delivered with input callbacks. VMNL preserves those bits in keyboard and mouse-button
    /// events.
    ///
    /// # Arguments
    ///
    /// - `enabled` controls whether Caps Lock and Num Lock are included in event modifiers.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self::set_cursor`].
    #[inline]
    pub fn set_lock_key_modifier_reporting(&mut self, enabled: bool) {
        self.inner.set_lock_key_modifier_reporting(enabled);
    }

    /// Returns whether raw mouse motion is configured for this window.
    ///
    /// A `true` value records the option; raw deltas are delivered only while the cursor mode is
    /// [`CursorMode::Disabled`].
    ///
    /// # Example
    ///
    /// See the workflow on [`Self::set_cursor`].
    #[inline]
    #[must_use]
    pub fn is_raw_mouse_motion_enabled(&self) -> bool {
        self.inner.is_raw_mouse_motion_enabled()
    }

    /// Enables or disables raw, unscaled and unaccelerated mouse motion for this window.
    ///
    /// The option can be configured independently of the cursor mode, but only affects cursor
    /// motion while [`CursorMode::Disabled`] is effective.
    ///
    /// # Arguments
    ///
    /// - `enabled` requests unscaled and unaccelerated motion when true.
    ///
    /// # Errors
    ///
    /// Returns [`GlfwUnsupportedPlatform`](crate::VMNLErrorKind::GlfwUnsupportedPlatform) when
    /// enabling raw motion on a system where
    /// [`Context::is_raw_mouse_motion_supported`](crate::Context::is_raw_mouse_motion_supported)
    /// is `false`. Disabling remains a successful no-op on such systems.
    ///
    /// # Example
    ///
    /// See the workflow on [`Self::set_cursor`].
    #[inline]
    pub fn set_raw_mouse_motion(&mut self, enabled: bool) -> VMNLResult<()> {
        self.inner.set_raw_mouse_motion(enabled)
    }
}
