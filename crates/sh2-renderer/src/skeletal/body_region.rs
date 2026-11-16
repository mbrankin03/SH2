//! Anatomical body-region classification and First-Person cranial triangle partitioning for
//! skeletal character models (`SH2-93`).
//!
//! In `FirstPerson` the foreground player hides **only** the cranial head subtree (skull, face,
//! jaw, eyes, eyelashes, hair) while the neck stump, collar, jacket/blouse, arms, hands, legs and
//! shoes stay visible. Triangles are partitioned per-vertex (not per-primitive) because retail
//! head primitives also carry the neck skin (`lll_jms.mdl` prim 1: bones `[3, 8, 15]`), and
//! retail torso primitives must never be dropped wholesale (the `SH2-93` playtest "hollow waist").

use sh2_assets::formats::mdls::ModelFile;

/// Anatomical body region of a character sub-primitive (`SH2-93`), classified from bind-pose bone
/// heights. Used to locate the cranial root bone and by diagnostics; draw filtering itself is
/// per-triangle via [`partition_first_person_triangles`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkeletalBodyRegion {
    /// Pelvis, skirt, legs, feet, and forearm/hand pose variants (`Y > -630.0` in bind pose).
    #[default]
    LowerOrHands,
    /// Upper spine, jacket/shirt torso, lapels, clavicles, and shoulders (`-742.0 < Y <= -630.0`).
    UpperTorso,
    /// Neck, head, jaw, eyes, eyelashes, and opaque/transparent hair fringes (`Y <= -742.0`).
    HeadAndHair,
}

/// Classifies a [`ModelFile`] primitive into [`SkeletalBodyRegion`] from its active bone bind-pose
/// heights (`initial_matrices[b][1][3]` in `+Y`-down SH2 world units, `500.0 units = 1.0 m`) (`SH2-93`).
///
/// Handles both opaque primitives (`pr.bone_indices`) and transparent hair/eyelash primitives
/// (whose `pr.bone_indices` are empty and store resolved bone IDs per vertex in `model.vertices.bone_indices`).
pub fn classify_primitive_body_region(
    model: &ModelFile,
    prim_index: usize,
    tris: &[[u16; 3]],
) -> SkeletalBodyRegion {
    let Some(pr) = model.primitives.get(prim_index) else {
        return SkeletalBodyRegion::LowerOrHands;
    };
    // Hand pose variants (`pose_index` 3..=20 in James / Maria) are always kept in LowerOrHands
    if (3..=20).contains(&pr.pose_index) {
        return SkeletalBodyRegion::LowerOrHands;
    }

    let bind_y = |b: usize| model.initial_matrices.get(b).map(|m| m[1][3]);
    let from_prim_bones = pr
        .bone_indices
        .iter()
        .filter_map(|&b| bind_y(b as usize))
        .reduce(f32::min);
    let min_bind_y = from_prim_bones.or_else(|| {
        tris.iter()
            .flatten()
            .filter_map(|&vi| model.vertices.bone_indices.get(vi as usize))
            .filter_map(|bones| bind_y(bones[0] as usize))
            .reduce(f32::min)
    });

    match min_bind_y {
        None => SkeletalBodyRegion::LowerOrHands,
        Some(y) if y <= -742.0 => SkeletalBodyRegion::HeadAndHair,
        Some(y) if y <= -630.0 => SkeletalBodyRegion::UpperTorso,
        Some(_) => SkeletalBodyRegion::LowerOrHands,
    }
}

/// Returns the global bone index carrying the largest blend weight for vertex `vertex_index`,
/// or `None` if the vertex is out of range.
pub fn dominant_vertex_bone(model: &ModelFile, vertex_index: usize) -> Option<usize> {
    let bones = model.vertices.bone_indices.get(vertex_index)?;
    let weights = model.vertices.weights.get(vertex_index)?;
    let best = (1..4).fold(
        0,
        |best, j| if weights[j] > weights[best] { j } else { best },
    );
    Some(bones[best] as usize)
}

/// Locates the cranial root bone (`15` on `lll_jms`/`hll_jms`/`hhh_jms`, `9` on `lll_mar`/`hhh_mar`)
/// as the most frequent dominant bone across all [`SkeletalBodyRegion::HeadAndHair`] primitives.
pub fn find_cranial_root_bone(model: &ModelFile) -> Option<usize> {
    let mut votes = vec![0usize; model.bone_count];
    for k in 0..model.primitives.len() {
        let tris = model.primitive_triangles(k);
        if classify_primitive_body_region(model, k, &tris) != SkeletalBodyRegion::HeadAndHair {
            continue;
        }
        for &vi in tris.iter().flatten() {
            if let Some(slot) =
                dominant_vertex_bone(model, vi as usize).and_then(|b| votes.get_mut(b))
            {
                *slot += 1;
            }
        }
    }
    votes
        .iter()
        .enumerate()
        .filter(|(_, &n)| n > 0)
        .max_by_key(|(_, &n)| n)
        .map(|(b, _)| b)
}

/// Builds a per-bone mask flagging the cranial root bone and every descendant (eyes, jaw, hair
/// physics chains) by walking `model.parents`. Returns an all-`false` mask for headless models.
pub fn cranial_bone_mask(model: &ModelFile) -> Vec<bool> {
    let n = model.bone_count;
    let mut mask = vec![false; n];
    let Some(root) = find_cranial_root_bone(model) else {
        return mask;
    };
    for (b, flag) in mask.iter_mut().enumerate() {
        let mut cur = b;
        // A valid skeleton reaches its root in at most `n` hops; the bound guards malformed cycles.
        for _ in 0..n {
            if cur == root {
                *flag = true;
                break;
            }
            match model.parents.get(cur).map(|&p| p as usize) {
                Some(p) if p < n && p != cur => cur = p,
                _ => break,
            }
        }
    }
    mask
}

/// Partitions `tris` into `(below_neck, cranial)` triangle lists for First-Person rendering.
///
/// A triangle is cranial if **any** of its vertices is dominated by a bone in `cranial_mask`, so
/// no head/face/hair vertex is ever drawn in front of the First-Person eye while the neck stump
/// (`lll_jms` bones `3`/`8`), collar, torso, arms and legs are preserved.
pub fn partition_first_person_triangles(
    model: &ModelFile,
    tris: &[[u16; 3]],
    cranial_mask: &[bool],
) -> (Vec<[u16; 3]>, Vec<[u16; 3]>) {
    let is_cranial = |vi: u16| {
        dominant_vertex_bone(model, vi as usize)
            .and_then(|b| cranial_mask.get(b).copied())
            .unwrap_or(false)
    };
    tris.iter()
        .copied()
        .partition(|tri| !tri.iter().any(|&vi| is_cranial(vi)))
}
