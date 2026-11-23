// SPDX-FileCopyrightText: 2026 Hugo Duda
// SPDX-License-Identifier: MIT

//! SPIR-V descriptor and sampling contracts for the raw graphics API.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use vulkano::descriptor_set::layout::DescriptorType;
use vulkano::shader::spirv::{Decoration, ExecutionModel, Id, Instruction, Spirv, StorageClass};
use vulkano::shader::{reflect, DescriptorBindingRequirements, DescriptorIdentifier};

use crate::exception::{VMNLError, VMNLErrorKind, VMNLResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct BindingPoint {
    pub(super) set: u32,
    pub(super) binding: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct SampledPair {
    pub(super) image: BindingPoint,
    pub(super) sampler: BindingPoint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SamplerOperation {
    ExplicitLodZero,
    DepthComparison,
    TexelIncompatible(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SamplerUse {
    pub(super) pair: SampledPair,
    pub(super) operation: SamplerOperation,
}

#[derive(Clone, Debug)]
pub(super) struct ShaderStageContract {
    pub(super) descriptors: BTreeMap<BindingPoint, DescriptorBindingRequirements>,
    pub(super) descriptor_arrays: BTreeSet<BindingPoint>,
    pub(super) sampler_uses: Vec<SamplerUse>,
    #[cfg(test)]
    function_visits: usize,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ShaderContract {
    pub(super) descriptors: BTreeMap<BindingPoint, DescriptorBindingRequirements>,
    pub(super) sampled_pairs: BTreeSet<SampledPair>,
    pub(super) sampler_uses: Vec<SamplerUse>,
}

impl ShaderContract {
    /// Combines reflected vertex and fragment resource requirements by descriptor location.
    ///
    /// The stage contracts must agree on the type for any shared descriptor binding.
    pub(super) fn merge(
        vertex: &ShaderStageContract,
        fragment: &ShaderStageContract,
    ) -> VMNLResult<Self> {
        let mut descriptors = vertex.descriptors.clone();
        for (&point, fragment_requirements) in &fragment.descriptors {
            if let Some(vertex_requirements) = descriptors.get_mut(&point) {
                vertex_requirements
                    .merge(fragment_requirements)
                    .map_err(|error| {
                        invalid_shader(format!(
                            "raw pipeline shader stages conflict at set {} binding {}: {error}",
                            point.set, point.binding
                        ))
                    })?;
            } else {
                descriptors.insert(point, fragment_requirements.clone());
            }
        }

        let mut descriptor_arrays = vertex.descriptor_arrays.clone();
        descriptor_arrays.extend(fragment.descriptor_arrays.iter().copied());
        if let Some(point) = descriptor_arrays.first() {
            return Err(invalid_shader(format!(
                "raw pipeline set {} binding {} descriptor arrays are not supported yet",
                point.set, point.binding
            )));
        }

        let mut contract = Self {
            descriptors,
            ..Default::default()
        };
        contract
            .sampler_uses
            .extend(vertex.sampler_uses.iter().copied());
        contract
            .sampler_uses
            .extend(fragment.sampler_uses.iter().copied());
        contract
            .sampled_pairs
            .extend(contract.sampler_uses.iter().map(|sample| sample.pair));

        validate_reflected_pairs(&contract, vertex)?;
        validate_reflected_pairs(&contract, fragment)?;

        Ok(contract)
    }
}

/// Extracts resource declarations and sampling requirements for a shader stage.
///
/// Use SPIR-V from the compiled stage and the matching execution model.
pub(super) fn reflect_stage_contract(
    words: &[u32],
    execution_model: ExecutionModel,
    stage_name: &str,
) -> VMNLResult<ShaderStageContract> {
    let spirv = Spirv::new(words)
        .map_err(|_| VMNLError::new(VMNLErrorKind::VulkanShaderModuleCreationFailed))?;
    let (entry_point, entry_info) = reflect::entry_points(&spirv)
        .find(|(_, info)| info.name == "main" && info.execution_model == execution_model)
        .ok_or_else(|| VMNLError::new(VMNLErrorKind::VulkanShaderCompilationFailed))?;

    let mut descriptors = BTreeMap::new();
    for (&(set, binding), requirements) in &entry_info.descriptor_binding_requirements {
        descriptors.insert(BindingPoint { set, binding }, requirements.clone());
    }

    let global_descriptors = global_descriptor_resources(&spirv);
    let mut descriptor_arrays = BTreeSet::new();
    for resource in global_descriptors.values() {
        if resource.is_array && descriptors.contains_key(&resource.point) {
            descriptor_arrays.insert(resource.point);
        }
    }

    let mut analysis = FunctionAnalysis::default();
    analyze_function(
        &spirv,
        entry_point,
        Vec::new(),
        &global_descriptors,
        stage_name,
        &mut analysis,
    )?;

    let stage = ShaderStageContract {
        descriptors,
        descriptor_arrays,
        sampler_uses: analysis.sampler_uses,
        #[cfg(test)]
        function_visits: analysis.function_visits,
    };
    validate_reflected_pairs_for_stage(&stage)?;
    Ok(stage)
}

#[derive(Clone, Copy, Debug)]
struct GlobalDescriptor {
    point: BindingPoint,
    kind: DescriptorKind,
    is_array: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DescriptorKind {
    SampledImage,
    Sampler,
    CombinedImageSampler,
    Other,
}

fn global_descriptor_resources(spirv: &Spirv) -> HashMap<Id, GlobalDescriptor> {
    let mut descriptors = HashMap::new();
    for instruction in spirv.global_variables() {
        let Instruction::Variable {
            result_id,
            result_type_id,
            storage_class: StorageClass::UniformConstant,
            ..
        } = *instruction
        else {
            continue;
        };

        let Some(point) = descriptor_point(spirv, result_id) else {
            continue;
        };
        let (kind, is_array) = descriptor_kind(spirv, result_type_id);
        descriptors.insert(
            result_id,
            GlobalDescriptor {
                point,
                kind,
                is_array,
            },
        );
    }
    descriptors
}

fn descriptor_point(spirv: &Spirv, variable: Id) -> Option<BindingPoint> {
    let mut set = None;
    let mut binding = None;
    for decoration in spirv.id(variable).decorations() {
        match decoration {
            Instruction::Decorate {
                decoration: Decoration::DescriptorSet { descriptor_set },
                ..
            } => set = Some(*descriptor_set),
            Instruction::Decorate {
                decoration: Decoration::Binding { binding_point },
                ..
            } => binding = Some(*binding_point),
            _ => {}
        }
    }
    Some(BindingPoint {
        set: set?,
        binding: binding?,
    })
}

fn descriptor_kind(spirv: &Spirv, pointer_type: Id) -> (DescriptorKind, bool) {
    let Instruction::TypePointer {
        ty: mut type_id, ..
    } = *spirv.id(pointer_type).instruction()
    else {
        return (DescriptorKind::Other, false);
    };
    let mut is_array = false;
    loop {
        match *spirv.id(type_id).instruction() {
            Instruction::TypeArray { element_type, .. }
            | Instruction::TypeRuntimeArray { element_type, .. } => {
                is_array = true;
                type_id = element_type;
            }
            Instruction::TypeSampledImage { .. } => {
                return (DescriptorKind::CombinedImageSampler, is_array)
            }
            Instruction::TypeSampler { .. } => return (DescriptorKind::Sampler, is_array),
            Instruction::TypeImage { sampled: 1, .. } => {
                return (DescriptorKind::SampledImage, is_array)
            }
            _ => return (DescriptorKind::Other, is_array),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
struct ResourceValue {
    images: BTreeSet<BindingPoint>,
    samplers: BTreeSet<BindingPoint>,
    sampled_pairs: BTreeSet<SampledPair>,
    unknown: bool,
}

impl ResourceValue {
    fn merge(&mut self, other: Self) {
        self.images.extend(other.images);
        self.samplers.extend(other.samplers);
        self.sampled_pairs.extend(other.sampled_pairs);
        self.unknown |= other.unknown;
    }

    fn for_descriptor(resource: GlobalDescriptor) -> Self {
        let mut value = Self::default();
        match resource.kind {
            DescriptorKind::SampledImage => {
                value.images.insert(resource.point);
            }
            DescriptorKind::Sampler => {
                value.samplers.insert(resource.point);
            }
            DescriptorKind::CombinedImageSampler => {
                value.sampled_pairs.insert(SampledPair {
                    image: resource.point,
                    sampler: resource.point,
                });
            }
            DescriptorKind::Other => value.unknown = true,
        }
        value
    }

    fn image_operand_value(&self) -> Self {
        let mut value = Self {
            images: self.images.clone(),
            unknown: self.unknown,
            ..Default::default()
        };
        value
            .images
            .extend(self.sampled_pairs.iter().map(|pair| pair.image));
        value
    }

    fn combine_image_and_sampler(image: Self, sampler: Self) -> Self {
        let mut images = image.images;
        images.extend(image.sampled_pairs.iter().map(|pair| pair.image));
        let mut samplers = sampler.samplers;
        samplers.extend(sampler.sampled_pairs.iter().map(|pair| pair.sampler));

        let mut value = Self {
            unknown: image.unknown || sampler.unknown,
            ..Default::default()
        };
        for image in images {
            for sampler in &samplers {
                value.sampled_pairs.insert(SampledPair {
                    image,
                    sampler: *sampler,
                });
            }
        }
        if value.sampled_pairs.is_empty() {
            value.unknown = true;
        }
        value
    }
}

fn resolve_resource_value(
    spirv: &Spirv,
    id: Id,
    parameters: &HashMap<Id, ResourceValue>,
    call_results: &HashMap<Id, ResourceValue>,
    descriptors: &HashMap<Id, GlobalDescriptor>,
    visited: &mut HashSet<Id>,
) -> ResourceValue {
    if let Some(value) = parameters.get(&id).or_else(|| call_results.get(&id)) {
        return value.clone();
    }
    if let Some(descriptor) = descriptors.get(&id) {
        return ResourceValue::for_descriptor(*descriptor);
    }
    if !visited.insert(id) {
        return ResourceValue {
            unknown: true,
            ..Default::default()
        };
    }

    let value = match *spirv.id(id).instruction() {
        Instruction::Load { pointer, .. } => resolve_resource_value(
            spirv,
            pointer,
            parameters,
            call_results,
            descriptors,
            visited,
        ),
        Instruction::AccessChain { base, .. }
        | Instruction::InBoundsAccessChain { base, .. }
        | Instruction::PtrAccessChain { base, .. } => {
            resolve_resource_value(spirv, base, parameters, call_results, descriptors, visited)
        }
        Instruction::CopyObject { operand, .. } => resolve_resource_value(
            spirv,
            operand,
            parameters,
            call_results,
            descriptors,
            visited,
        ),
        Instruction::SampledImage { image, sampler, .. } => {
            let image = resolve_resource_value(
                spirv,
                image,
                parameters,
                call_results,
                descriptors,
                visited,
            );
            let sampler = resolve_resource_value(
                spirv,
                sampler,
                parameters,
                call_results,
                descriptors,
                visited,
            );
            ResourceValue::combine_image_and_sampler(image, sampler)
        }
        Instruction::Image { sampled_image, .. } => resolve_resource_value(
            spirv,
            sampled_image,
            parameters,
            call_results,
            descriptors,
            visited,
        )
        .image_operand_value(),
        Instruction::Phi {
            ref variable_parent,
            ..
        } => merge_resource_values(
            spirv,
            variable_parent.iter().map(|(incoming, _)| *incoming),
            parameters,
            call_results,
            descriptors,
            visited,
        ),
        Instruction::Select {
            object_1, object_2, ..
        } => merge_resource_values(
            spirv,
            [object_1, object_2],
            parameters,
            call_results,
            descriptors,
            visited,
        ),
        _ => ResourceValue {
            unknown: true,
            ..Default::default()
        },
    };

    visited.remove(&id);
    value
}

fn merge_resource_values(
    spirv: &Spirv,
    ids: impl IntoIterator<Item = Id>,
    parameters: &HashMap<Id, ResourceValue>,
    call_results: &HashMap<Id, ResourceValue>,
    descriptors: &HashMap<Id, GlobalDescriptor>,
    visited: &mut HashSet<Id>,
) -> ResourceValue {
    ids.into_iter()
        .fold(ResourceValue::default(), |mut value, id| {
            value.merge(resolve_resource_value(
                spirv,
                id,
                parameters,
                call_results,
                descriptors,
                visited,
            ));
            value
        })
}

#[derive(Default)]
struct FunctionAnalysis {
    call_stack: HashSet<Id>,
    // Scoped to this stage. A completed call already contributed its sampler uses.
    completed: HashMap<(Id, Vec<ResourceValue>), ResourceValue>,
    sampler_uses: Vec<SamplerUse>,
    #[cfg(test)]
    function_visits: usize,
}

fn analyze_function(
    spirv: &Spirv,
    function_id: Id,
    arguments: Vec<ResourceValue>,
    descriptors: &HashMap<Id, GlobalDescriptor>,
    stage_name: &str,
    analysis: &mut FunctionAnalysis,
) -> VMNLResult<ResourceValue> {
    if analysis.call_stack.contains(&function_id) {
        return Err(invalid_shader(format!(
            "raw {stage_name} shader uses recursive functions in descriptor sampling"
        )));
    }

    let key = (function_id, arguments);
    if let Some(returned) = analysis.completed.get(&key) {
        return Ok(returned.clone());
    }
    analysis.call_stack.insert(function_id);

    #[cfg(test)]
    {
        analysis.function_visits += 1;
    }

    let function = spirv.function(function_id);
    let parameter_ids: Vec<_> = function
        .instructions()
        .iter()
        .filter_map(|instruction| match instruction {
            Instruction::FunctionParameter { result_id, .. } => Some(*result_id),
            _ => None,
        })
        .collect();
    if parameter_ids.len() != key.1.len() {
        analysis.call_stack.remove(&function_id);
        return Err(invalid_shader(format!(
            "raw {stage_name} shader function parameter reflection is inconsistent"
        )));
    }
    let parameters: HashMap<_, _> = parameter_ids
        .into_iter()
        .zip(key.1.iter().cloned())
        .collect();

    let mut call_results = HashMap::new();
    let mut return_value = ResourceValue::default();
    for instruction in function.instructions() {
        match instruction {
            Instruction::FunctionCall {
                result_id,
                function: called_function,
                arguments,
                ..
            } => {
                let arguments = arguments
                    .iter()
                    .map(|argument| {
                        resolve_resource_value(
                            spirv,
                            *argument,
                            &parameters,
                            &call_results,
                            descriptors,
                            &mut HashSet::new(),
                        )
                    })
                    .collect::<Vec<_>>();
                let returned = analyze_function(
                    spirv,
                    *called_function,
                    arguments,
                    descriptors,
                    stage_name,
                    analysis,
                )?;
                call_results.insert(*result_id, returned);
            }
            Instruction::ReturnValue { value } => {
                return_value.merge(resolve_resource_value(
                    spirv,
                    *value,
                    &parameters,
                    &call_results,
                    descriptors,
                    &mut HashSet::new(),
                ));
            }
            _ => analyze_image_operation(
                spirv,
                instruction,
                &parameters,
                &call_results,
                descriptors,
                stage_name,
                &mut analysis.sampler_uses,
            )?,
        }
    }

    analysis.call_stack.remove(&function_id);
    // The entry point has no caller, so it is never reused.
    if !analysis.call_stack.is_empty() {
        analysis.completed.insert(key, return_value.clone());
    }
    Ok(return_value)
}

fn analyze_image_operation(
    spirv: &Spirv,
    instruction: &Instruction,
    parameters: &HashMap<Id, ResourceValue>,
    call_results: &HashMap<Id, ResourceValue>,
    descriptors: &HashMap<Id, GlobalDescriptor>,
    stage_name: &str,
    sampler_uses: &mut Vec<SamplerUse>,
) -> VMNLResult<()> {
    if let Instruction::ImageFetch { image, .. } | Instruction::ImageSparseFetch { image, .. } =
        instruction
    {
        let value = resolve_resource_value(
            spirv,
            *image,
            parameters,
            call_results,
            descriptors,
            &mut HashSet::new(),
        );
        let Some(point) = value.images.first().filter(|_| !value.unknown) else {
            return Err(invalid_shader(format!(
                "raw {stage_name} shader samplerless texelFetch is outside the sampling contract and its image binding could not be verified"
            )));
        };
        return Err(invalid_shader(format!(
            "raw {stage_name} shader samplerless texelFetch at image set {} binding {} is outside the sampling contract",
            point.set, point.binding
        )));
    }

    let Some((sampled_image, operation)) = sampling_instruction(spirv, instruction) else {
        return Ok(());
    };
    let value = resolve_resource_value(
        spirv,
        sampled_image,
        parameters,
        call_results,
        descriptors,
        &mut HashSet::new(),
    );
    if value.unknown || value.sampled_pairs.is_empty() {
        return Err(invalid_shader(format!(
            "raw {stage_name} shader sampling image/sampler origins could not be verified"
        )));
    }
    sampler_uses.extend(
        value
            .sampled_pairs
            .into_iter()
            .map(|pair| SamplerUse { pair, operation }),
    );
    Ok(())
}

fn sampling_instruction(
    spirv: &Spirv,
    instruction: &Instruction,
) -> Option<(Id, SamplerOperation)> {
    let explicit_lod = |sampled_image: Id, operands: &vulkano::shader::spirv::ImageOperands| {
        let lod_is_zero = operands
            .lod
            .is_some_and(|lod| is_float_zero_constant(spirv, lod));
        let extra_lod_is_zero = operands
            .min_lod
            .is_none_or(|lod| is_float_zero_constant(spirv, lod));
        let no_bias_or_offset = operands.bias.is_none()
            && operands.grad.is_none()
            && operands.const_offset.is_none()
            && operands.offset.is_none()
            && operands.const_offsets.is_none()
            && operands.offsets.is_none();
        let operation = if lod_is_zero && extra_lod_is_zero && no_bias_or_offset {
            SamplerOperation::ExplicitLodZero
        } else {
            SamplerOperation::TexelIncompatible(
                "texel coordinates require explicit LOD zero without bias or offsets",
            )
        };
        (sampled_image, operation)
    };

    match *instruction {
        Instruction::ImageSampleExplicitLod {
            sampled_image,
            ref image_operands,
            ..
        }
        | Instruction::ImageSparseSampleExplicitLod {
            sampled_image,
            ref image_operands,
            ..
        } => Some(explicit_lod(sampled_image, image_operands)),
        Instruction::ImageSampleImplicitLod {
            sampled_image,
            ref image_operands,
            ..
        }
        | Instruction::ImageSparseSampleImplicitLod {
            sampled_image,
            ref image_operands,
            ..
        } => Some((
            sampled_image,
            if image_operands.as_ref().is_some_and(|operands| {
                operands.bias.is_some()
                    || operands.const_offset.is_some()
                    || operands.offset.is_some()
                    || operands.const_offsets.is_some()
                    || operands.offsets.is_some()
            }) {
                SamplerOperation::TexelIncompatible(
                    "texel coordinates do not support bias or offsets",
                )
            } else {
                SamplerOperation::TexelIncompatible("texel coordinates do not support implicit LOD")
            },
        )),
        Instruction::ImageSampleDrefImplicitLod { sampled_image, .. }
        | Instruction::ImageSampleDrefExplicitLod { sampled_image, .. }
        | Instruction::ImageSampleProjDrefImplicitLod { sampled_image, .. }
        | Instruction::ImageSampleProjDrefExplicitLod { sampled_image, .. }
        | Instruction::ImageSparseSampleDrefImplicitLod { sampled_image, .. }
        | Instruction::ImageSparseSampleDrefExplicitLod { sampled_image, .. }
        | Instruction::ImageSparseSampleProjDrefImplicitLod { sampled_image, .. }
        | Instruction::ImageSparseSampleProjDrefExplicitLod { sampled_image, .. }
        | Instruction::ImageDrefGather { sampled_image, .. }
        | Instruction::ImageSparseDrefGather { sampled_image, .. } => {
            Some((sampled_image, SamplerOperation::DepthComparison))
        }
        Instruction::ImageSampleProjImplicitLod { sampled_image, .. }
        | Instruction::ImageSampleProjExplicitLod { sampled_image, .. }
        | Instruction::ImageSparseSampleProjImplicitLod { sampled_image, .. }
        | Instruction::ImageSparseSampleProjExplicitLod { sampled_image, .. } => Some((
            sampled_image,
            SamplerOperation::TexelIncompatible("texel coordinates do not support projection"),
        )),
        Instruction::ImageGather { sampled_image, .. }
        | Instruction::ImageSparseGather { sampled_image, .. } => Some((
            sampled_image,
            SamplerOperation::TexelIncompatible(
                "texel coordinates do not support gather operations",
            ),
        )),
        _ => None,
    }
}

fn is_float_zero_constant(spirv: &Spirv, id: Id) -> bool {
    let Instruction::Constant {
        result_type_id,
        ref value,
        ..
    } = *spirv.id(id).instruction()
    else {
        return false;
    };
    matches!(
        *spirv.id(result_type_id).instruction(),
        Instruction::TypeFloat { width: 32, .. }
    ) && value.len() == 1
        && f32::from_bits(value[0]) == 0.0
}

fn validate_reflected_pairs_for_stage(stage: &ShaderStageContract) -> VMNLResult<()> {
    for (&sampler, requirements) in &stage.descriptors {
        for descriptor in requirements.descriptors.values() {
            for image in &descriptor.sampler_with_images {
                let pair = SampledPair {
                    image: BindingPoint {
                        set: image.set,
                        binding: image.binding,
                    },
                    sampler,
                };
                if !stage.sampler_uses.iter().any(|usage| usage.pair == pair) {
                    return Err(invalid_shader(format!(
                        "raw shader cannot verify sampled image/sampler pair at image set {} binding {} and sampler set {} binding {}",
                        pair.image.set,
                        pair.image.binding,
                        pair.sampler.set,
                        pair.sampler.binding
                    )));
                }
            }
        }
    }
    Ok(())
}

fn validate_reflected_pairs(
    contract: &ShaderContract,
    stage: &ShaderStageContract,
) -> VMNLResult<()> {
    validate_reflected_pairs_for_stage(stage)?;
    for (&sampler, requirements) in &stage.descriptors {
        for descriptor in requirements.descriptors.values() {
            for DescriptorIdentifier {
                set: image_set,
                binding: image_binding,
                ..
            } in &descriptor.sampler_with_images
            {
                if !contract.sampled_pairs.contains(&SampledPair {
                    image: BindingPoint {
                        set: *image_set,
                        binding: *image_binding,
                    },
                    sampler,
                }) {
                    return Err(invalid_shader(format!(
                        "raw pipeline cannot verify sampled image/sampler pair at image set {image_set} binding {image_binding} and sampler set {} binding {}",
                        sampler.set, sampler.binding
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Checks whether the descriptor type is supported by VMNL raw bindings.
///
/// The descriptor coordinates identify the rejected declaration in the error.
pub(super) fn validate_raw_descriptor_type(
    set: u32,
    binding: u32,
    descriptor_type: DescriptorType,
) -> VMNLResult<()> {
    if !matches!(
        descriptor_type,
        DescriptorType::UniformBuffer
            | DescriptorType::CombinedImageSampler
            | DescriptorType::SampledImage
            | DescriptorType::Sampler
    ) {
        return Err(invalid_shader(format!(
            "raw pipeline set {set} binding {binding} only supports uniform buffers, sampled images, samplers, and combined image samplers for now"
        )));
    }
    Ok(())
}

fn invalid_shader(message: impl Into<String>) -> VMNLError {
    VMNLError::new(VMNLErrorKind::InvalidState(message.into()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use super::*;

    fn reflect_fragment(source: &str) -> ShaderStageContract {
        let compiler = shaderc::Compiler::new().expect("shaderc compiler should initialize");
        let artifact = compiler
            .compile_into_spirv(
                source,
                shaderc::ShaderKind::Fragment,
                "raw_shader_compat_test.frag",
                "main",
                None,
            )
            .expect("fragment shader should compile");
        reflect_stage_contract(artifact.as_binary(), ExecutionModel::Fragment, "fragment")
            .expect("shader reflection should succeed")
    }

    fn fragment_shader(body: &str, declarations: &str) -> String {
        format!(
            "#version 450\n{declarations}\nlayout(location = 0) in vec2 uv;\nlayout(location = 0) out vec4 color;\nvoid main() {{ color = {body}; }}"
        )
    }

    #[test]
    fn repeated_calls_only_analyze_each_non_texture_helper_once() {
        use std::fmt::Write;

        const DEPTH: usize = 12;
        let mut helpers = String::from("vec4 level0(vec2 uv) { return vec4(uv, 0.0, 1.0); }\n");
        for level in 1..=DEPTH {
            let next = level - 1;
            writeln!(
                helpers,
                "vec4 level{level}(vec2 uv) {{ return level{next}(uv) + level{next}(uv * 0.5); }}"
            )
            .expect("formatting a String should succeed");
        }
        let contract = reflect_fragment(&fragment_shader(&format!("level{DEPTH}(uv)"), &helpers));

        assert!(contract.descriptors.is_empty());
        assert!(contract.sampler_uses.is_empty());
        assert_eq!(contract.function_visits, DEPTH + 2); // main plus level0 through level12
    }

    #[test]
    fn separate_descriptor_pairs_follow_helper_parameters_across_sets() {
        let shader = r"
            #version 450
            layout(set = 0, binding = 3) uniform texture2D color_image;
            layout(set = 1, binding = 2) uniform sampler nearest_sampler;
            layout(set = 4, binding = 7) uniform sampler linear_sampler;
            layout(location = 0) in vec2 uv;
            layout(location = 0) out vec4 color;
            vec4 sample_with(texture2D image, sampler state, vec2 coordinates) {
                return texture(sampler2D(image, state), coordinates);
            }
            vec4 sample_at_lod(texture2D image, sampler state, vec2 coordinates) {
                return textureLod(sampler2D(image, state), coordinates, 0.0);
            }
            void main() {
                color = sample_with(color_image, nearest_sampler, uv)
                    + sample_with(color_image, linear_sampler, uv)
                    + sample_with(color_image, nearest_sampler, uv * 0.5)
                    + sample_at_lod(color_image, linear_sampler, uv);
            }
        ";
        let contract = reflect_fragment(shader);
        assert_eq!(contract.function_visits, 4);
        assert_eq!(contract.sampler_uses.len(), 3);
        let pairs: BTreeSet<_> = contract
            .sampler_uses
            .iter()
            .map(|usage| usage.pair)
            .collect();

        assert_eq!(
            pairs,
            BTreeSet::from([
                SampledPair {
                    image: BindingPoint { set: 0, binding: 3 },
                    sampler: BindingPoint { set: 1, binding: 2 },
                },
                SampledPair {
                    image: BindingPoint { set: 0, binding: 3 },
                    sampler: BindingPoint { set: 4, binding: 7 },
                },
            ])
        );
        assert!(contract
            .sampler_uses
            .iter()
            .any(|usage| usage.operation == SamplerOperation::ExplicitLodZero));
    }

    #[test]
    fn texel_sampling_reflection_distinguishes_explicit_constant_zero_lod() {
        let declarations = "layout(set = 0, binding = 0) uniform sampler2D image;";
        let fixtures = [
            (
                "texture(image, uv)",
                SamplerOperation::TexelIncompatible(
                    "texel coordinates do not support implicit LOD",
                ),
            ),
            (
                "textureLod(image, uv, 0.0)",
                SamplerOperation::ExplicitLodZero,
            ),
            (
                "textureLod(image, uv, 1.0)",
                SamplerOperation::TexelIncompatible(
                    "texel coordinates require explicit LOD zero without bias or offsets",
                ),
            ),
            (
                "textureLod(image, uv, uv.x)",
                SamplerOperation::TexelIncompatible(
                    "texel coordinates require explicit LOD zero without bias or offsets",
                ),
            ),
            (
                "textureProj(image, vec3(uv, 1.0))",
                SamplerOperation::TexelIncompatible("texel coordinates do not support projection"),
            ),
            (
                "texture(image, uv, 0.5)",
                SamplerOperation::TexelIncompatible(
                    "texel coordinates do not support bias or offsets",
                ),
            ),
            (
                "textureOffset(image, uv, ivec2(0))",
                SamplerOperation::TexelIncompatible(
                    "texel coordinates do not support bias or offsets",
                ),
            ),
        ];

        for (expression, expected) in fixtures {
            let contract = reflect_fragment(&fragment_shader(expression, declarations));
            assert_eq!(contract.sampler_uses.len(), 1, "{expression}");
            assert_eq!(contract.sampler_uses[0].operation, expected, "{expression}");
        }
    }

    #[test]
    fn normalized_sampling_accepts_shader_computed_lod() {
        let shader = fragment_shader(
            "textureLod(image, uv, uv.x)",
            "layout(set = 0, binding = 0) uniform sampler2D image;",
        );
        let stage = reflect_fragment(&shader);

        assert_eq!(
            stage.sampler_uses[0].operation,
            SamplerOperation::TexelIncompatible(
                "texel coordinates require explicit LOD zero without bias or offsets"
            )
        );
        assert!(ShaderContract::merge(&stage, &stage).is_ok());
    }

    #[test]
    fn depth_sampling_and_gather_are_classified_as_comparisons() {
        let point = BindingPoint { set: 2, binding: 5 };
        for expression in [
            "vec4(texture(shadow_image, vec3(uv, 0.5)))",
            "textureGather(shadow_image, uv, 0.5)",
        ] {
            let shader = fragment_shader(
                expression,
                "layout(set = 2, binding = 5) uniform sampler2DShadow shadow_image;",
            );
            let contract = reflect_fragment(&shader);
            assert_eq!(
                contract.sampler_uses,
                [SamplerUse {
                    pair: SampledPair {
                        image: point,
                        sampler: point,
                    },
                    operation: SamplerOperation::DepthComparison,
                }],
                "{expression}"
            );
        }
    }

    #[test]
    fn descriptor_array_of_length_one_remains_rejected() {
        let shader = fragment_shader(
            "texture(images[0], uv)",
            "layout(set = 0, binding = 0) uniform sampler2D images[1];",
        );
        let contract = reflect_fragment(&shader);
        assert!(contract
            .descriptor_arrays
            .contains(&BindingPoint { set: 0, binding: 0 }));
        assert!(ShaderContract::merge(&contract, &contract).is_err());
    }

    #[test]
    fn samplerless_texel_fetch_is_rejected_with_its_image_binding() {
        let shader = fragment_shader(
            "texelFetch(image, ivec2(0), 0)",
            "layout(set = 3, binding = 4) uniform sampler2D image;",
        );
        let compiler = shaderc::Compiler::new().expect("shaderc compiler should initialize");
        let artifact = compiler
            .compile_into_spirv(
                &shader,
                shaderc::ShaderKind::Fragment,
                "raw_shader_compat_test.frag",
                "main",
                None,
            )
            .expect("texelFetch shader should compile");

        let error =
            reflect_stage_contract(artifact.as_binary(), ExecutionModel::Fragment, "fragment")
                .expect_err("samplerless texelFetch should be outside the sampling contract");
        match error.kind() {
            VMNLErrorKind::InvalidState(message) => {
                assert!(message.contains("samplerless texelFetch"));
                assert!(message.contains("image set 3 binding 4"));
            }
            kind => panic!("expected InvalidState, received {kind:?}"),
        }
    }

    #[test]
    fn cross_stage_descriptor_conflict_returns_before_vulkano_layout_merge() {
        let vertex_source = r"
            #version 450
            layout(set = 0, binding = 0) uniform Transform { vec4 position; } transform;
            void main() { gl_Position = transform.position; }
        ";
        let compiler = shaderc::Compiler::new().expect("shaderc compiler should initialize");
        let artifact = compiler
            .compile_into_spirv(
                vertex_source,
                shaderc::ShaderKind::Vertex,
                "raw_shader_compat_test.vert",
                "main",
                None,
            )
            .expect("vertex shader should compile");
        let vertex = reflect_stage_contract(artifact.as_binary(), ExecutionModel::Vertex, "vertex")
            .expect("vertex reflection should succeed");
        let fragment = reflect_fragment(&fragment_shader(
            "texture(image, uv)",
            "layout(set = 0, binding = 0) uniform sampler2D image;",
        ));

        let error = ShaderContract::merge(&vertex, &fragment)
            .expect_err("conflicting stage descriptor types should be rejected");
        match error.kind() {
            VMNLErrorKind::InvalidState(message) => {
                assert!(message.contains("set 0 binding 0"));
            }
            kind => panic!("expected InvalidState, received {kind:?}"),
        }
    }
}
