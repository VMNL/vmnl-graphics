// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! Public raw derive-macro compatibility contracts.

use vmnl_graphics::{raw, FrameRenderer, VMNLResult};

#[repr(C)]
#[derive(Clone, Copy, raw::Vertex, raw::Pod, raw::Zeroable)]
struct PublicRawVertex {
    #[format(R32G32_SFLOAT)]
    position: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, raw::Pod, raw::Zeroable)]
struct PublicRawUniform {
    tint: [f32; 4],
}

fn assert_vertex<T: raw::Vertex + raw::Pod + raw::Zeroable>() {}
fn assert_uniform<T: raw::Pod + raw::Zeroable>() {}
fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn raw_resources_are_send_sync_for_external_clients() {
    assert_send_sync::<raw::Resources>();
    assert_send_sync::<raw::ResourcesBuilder>();
}

#[test]
fn raw_sampled_texture_binding_is_public_from_vmnl() -> VMNLResult<()> {
    // Check the external signature without creating a Vulkan device or texture.
    let _sampled_texture: fn(
        raw::ResourcesBuilder,
        u32,
        u32,
        &vmnl_graphics::texture::Texture2D,
        &vmnl_graphics::texture::Sampler,
    ) -> raw::ResourcesBuilder = raw::ResourcesBuilder::sampled_texture;

    Ok(())
}

#[test]
fn raw_separate_texture_and_sampler_bindings_are_public_from_vmnl() -> VMNLResult<()> {
    let _texture: fn(
        raw::ResourcesBuilder,
        u32,
        u32,
        &vmnl_graphics::texture::Texture2D,
    ) -> raw::ResourcesBuilder = raw::ResourcesBuilder::texture;
    let _sampler: fn(
        raw::ResourcesBuilder,
        u32,
        u32,
        &vmnl_graphics::texture::Sampler,
    ) -> raw::ResourcesBuilder = raw::ResourcesBuilder::sampler;
    let _sampled_texture_view: fn(
        raw::ResourcesBuilder,
        u32,
        u32,
        &vmnl_graphics::texture::TextureView2D,
        &vmnl_graphics::texture::Sampler,
    ) -> raw::ResourcesBuilder = raw::ResourcesBuilder::sampled_texture_view;
    let _texture_view: fn(
        raw::ResourcesBuilder,
        u32,
        u32,
        &vmnl_graphics::texture::TextureView2D,
    ) -> raw::ResourcesBuilder = raw::ResourcesBuilder::texture_view;

    Ok(())
}

#[test]
fn raw_derive_traits_are_usable_from_vmnl() -> VMNLResult<()> {
    assert_vertex::<PublicRawVertex>();
    assert_uniform::<PublicRawUniform>();
    Ok(())
}

#[test]
fn raw_uniform_write_is_public_from_vmnl() -> VMNLResult<()> {
    let _write: fn(&mut raw::Uniform<PublicRawUniform>, PublicRawUniform) -> VMNLResult<()> =
        raw::Uniform::<PublicRawUniform>::write;

    Ok(())
}

#[test]
fn raw_frame_uniform_api_is_public_from_vmnl() -> VMNLResult<()> {
    let _builder: fn(PublicRawUniform) -> raw::FrameUniformBuilder<PublicRawUniform> =
        raw::FrameUniform::<PublicRawUniform>::builder;
    let _bind: fn(
        raw::ResourcesBuilder,
        u32,
        u32,
        &raw::FrameUniform<PublicRawUniform>,
    ) -> raw::ResourcesBuilder = raw::ResourcesBuilder::frame_uniform::<PublicRawUniform>;
    fn assert_write_method<TData>()
    where
        TData: raw::BufferContents + 'static,
    {
        let _method = FrameRenderer::write_frame_uniform::<TData>;
    }

    assert_write_method::<PublicRawUniform>();

    Ok(())
}
