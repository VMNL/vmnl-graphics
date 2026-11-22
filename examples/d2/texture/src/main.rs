// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Minimal 2D texture creation workflow.

use vmnl_graphics::{
    texture::{Sampler, Texture2D, UploadWaitOutcome},
    Context, VMNLResult,
};

fn main() -> VMNLResult<()> {
    let context = Context::new()?;
    let texture =
        Texture2D::from_file(&context, "examples/d2/texture/assets/pikachu.png").build()?;
    let sampler = Sampler::builder(&context).build()?;

    // Validate the pairing and wait for the upload before reporting the texture as ready.
    texture.view().validate_sampler(&sampler)?;
    assert_eq!(
        texture.initial_upload().wait(None)?,
        UploadWaitOutcome::Complete
    );
    println!("Texture created : {:?}", texture.extent());
    Ok(())
}
