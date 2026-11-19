# Errors matrix

| Operation | Principal failure contract | Typical decision |
|---|---|---|
| `InputRuntime::acquire` / builder | GLFW initialization failure; explicit active-config conflict; active runtime owned by another thread | Use the resolved active setting or acquire/drop handles on the GLFW thread |
| `InputRuntime::sample_joystick` / mapping update | Backend callback/query failure (`GlfwInputOperationFailed`); rejected mapping bytes (`InvalidGamepadMapping`) | Check slot/backend state or correct the mapping text |
| `Context::new` | Vulkan initialization/device/queue/allocator categories | Abort graphics initialization or retry after environment change |
| `ContextBuilder::build` | initialization categories; `DeviceRequirementsNotMet` with the complete feature list for an unsatisfied nonempty request | Require a compatible GPU or explicitly change the requested configuration; requirements are never silently dropped |
| `WindowBuilder::build` | invalid size; GLFW/surface/swapchain/shader/render-target failures; unsupported strict present mode | Fix configuration or environment; choose preferred/portable present mode |
| `Pipeline::validate_target` | `InvalidState` for a different device or render-pass object, including another window with equal properties | Build a separate pipeline for that window; successful preflight does not validate geometry/resources or framebuffer readiness |
| Raw depth pipeline build | `InvalidState` before shader I/O for any enabled depth test without a depth attachment | Build the window with `RenderTargetConfig::depth`, or explicitly choose `DepthState::Disabled` |
| Render-target configuration/build/recreation | `InvalidState` for invalid clear depth/actual extent; `VulkanFormatNotSupported` for unsupported requested depth/sample configuration; device-query/image/view/framebuffer failures | Choose a finite clear value in `[0,1]` and a format supported by the selected device; no format or GPU fallback |
| Runtime window setters | invalid size/range/aspect (`InvalidWindowSize`/`InvalidState`) | Correct input before retry |
| Cursor position/raw-motion setters | non-finite coordinates (`InvalidState`); unsupported raw enable (`GlfwUnsupportedPlatform`) | Correct coordinates or keep raw motion disabled |
| `StickConfig` construction | dead zone outside finite `[0, 1)` (`InvalidState`) | Choose a finite threshold in `[0, 1)` |
| Cursor resource build/assign | invalid image/hotspot (`InvalidState`); unavailable theme shape (`GlfwUnsupportedPlatform`); native GLFW failure | Correct image data, choose another shape, or restore the default cursor |
| Pixel source preparation / PNG decode | invalid ranges or declarations (`InvalidState`); malformed PNG (`TextureDecodeFailed`); unsupported samples/metadata (`UnsupportedTextureSource`); CPU allocation (`TextureCpuAllocationFailed`) | Correct the source descriptor or convert the file to a supported static 8-bit PNG |
| Texture GPU build/upload | path I/O (`TextureSourceReadFailed`); invalid exact upload usage (`InvalidState`); image/view/staging/sampler creation; queue or fence errors (`TextureUploadFailed`) | Correct the source/request or resolve the device failure; upload errors persist through cloned tickets |
| Texture view/sampler compatibility or deferred collection | `InvalidState` for distinct `Context` identities, unsampleable images, unsupported linear filtering, or incompatible border numeric class; persistent upload failure during collection (`TextureUploadFailed`) | Build resources from one `Context`, select supported sampling settings, and collect again after resolving upload failure |
| Shape build | invalid/overflowing geometry; vertex/index allocation failure | Correct geometry or release/reduce resources |
| Circle-family build | `InvalidState` for invalid parameters, sector/cap mismatch, zero segments, or invalid topology/counts; generated coordinates are not rechecked and filled widths are not validated | Supply both sector and cap or neither; keep outline offsets finite/positive and generated geometry within usable bounds |
| Raw pipeline build | `InvalidLineWidth` / `DeviceFeatureNotEnabled` / portability point-polygon or viewport/scissor `InvalidState` before shader I/O; missing/read/compile/interface/layout/pipeline failures; `VulkanPipelineCreationFailed` if the selected blend state is unsupported by the device or color format | Use valid numeric values and viewport device limits; request required device features before context creation; choose a supported blend factor/operation and render-target format; correct shaders/layout |
| Viewport/scissor policy resolution | `InvalidState` for invalid viewport numbers/ranges/full dimensions or signed scissor sum overflow | Correct framebuffer-pixel values; headless resolution does not validate device viewport limits |
| Raw geometry/uniform build | invalid counts/indices or buffer allocation | Correct data/reduce allocation |
| Raw resources build | context/device/layout/set/binding/type/array/duplicate/missing mismatch; unsampleable texture or incompatible sampler; descriptor allocation | Align shader, resource usage, and sampler contract |
| Frame submit | device/render-pass/resource mismatch; `VulkanOutOfDate` during acquisition, presentation, or zero-size recreation; record/submit/present/device errors | Keep processing events and retry with the same window after its framebuffer is renderable; rebuild/terminate on hard device failure |
| GLFW callbacks/infallible setters | No typed result; routed through callback/default backend handling | Log/classify callback and reconcile observed state |
| `time::Clock` reads and transitions | Infallible; interval and accumulation arithmetic saturates at zero/`Duration::MAX` instead of wrapping or panicking | None |

`VMNLErrorKind` is non-exhaustive. This table groups public behavior; exact per-item variants remain in Rustdoc/reference pages.
