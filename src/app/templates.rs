//! Lossless template instances stay out of the canonical note list.

use std::collections::HashSet;

use super::types::{ChartDoc, Note, NoteTemplateSource};

pub fn duration(notes: &[Note]) -> f32 {
    notes
        .iter()
        .map(|note| {
            note.time
                + note.hold_duration.max(
                    note.slide
                        .iter()
                        .map(|slide| slide.slide_duration)
                        .fold(0.0, f32::max),
                )
                - 1.0
        })
        .fold(0.25, f32::max)
}

/// Stable IDs prevent judgments and authored events leaking between instances.
fn virtual_id(instance: &str, source: u64, used: &mut HashSet<u64>) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in instance.bytes().chain(source.to_le_bytes()) {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    hash |= 1 << 63;
    while !used.insert(hash) {
        hash = hash.wrapping_add(1) | (1 << 63);
    }
    hash
}

pub fn expand(chart: &ChartDoc) -> ChartDoc {
    let mut result = chart.clone();
    let mut used: HashSet<_> = chart.notes.iter().map(|note| note.id).collect();
    for instance in &chart.template_instances {
        let Some(template) = chart
            .templates
            .iter()
            .find(|t| t.id == instance.template_id)
        else {
            continue;
        };
        for source in &template.notes {
            let mut note = source.clone();
            note.id = virtual_id(&instance.instance_id, source.id, &mut used);
            note.time += instance.anchor_time - 1.0;
            note.template_source = Some(NoteTemplateSource {
                instance_id: instance.instance_id.clone(),
                template_id: template.id.clone(),
                template_version: template.version,
                source_note_id: source.id,
            });
            result.notes.push(note);
        }
    }
    result
        .notes
        .sort_by(|a, b| a.time.total_cmp(&b.time).then(a.id.cmp(&b.id)));
    super::maichart::recompute_each(&mut result.notes);
    result.template_instances.clear();
    result
}

pub fn same_inputs(a: &ChartDoc, b: &ChartDoc) -> bool {
    a.notes == b.notes
        && a.templates == b.templates
        && a.template_instances == b.template_instances
        && a.bpms == b.bpms
        && a.autoplay_overrides == b.autoplay_overrides
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::types::{TemplateDef, TemplateInstance};

    #[test]
    fn instances_have_stable_independent_ids_and_do_not_mutate_the_source() {
        let mut chart = crate::app::chart::fallback_chart();
        chart.notes.clear();
        chart.templates.push(TemplateDef {
            id: "t".into(),
            name: "T".into(),
            version: 1,
            notes: vec![Note {
                id: 1,
                time: 1.0,
                ..Default::default()
            }],
            duration: 1.0,
        });
        for i in 0..2 {
            chart.template_instances.push(TemplateInstance {
                instance_id: format!("i{i}"),
                template_id: "t".into(),
                template_version: 1,
                anchor_time: 2.0 + i as f32,
            });
        }
        let first = expand(&chart);
        assert_eq!(first.notes.len(), 2);
        assert_ne!(first.notes[0].id, first.notes[1].id);
        assert_eq!(first.notes, expand(&chart).notes);
        assert_eq!(first.notes[0].time, 2.0);
        assert!(chart.notes.is_empty());
        chart.templates[0].notes[0].lane = 7;
        let changed = expand(&chart);
        assert!(changed.notes.iter().all(|note| note.lane == 7));
        assert_eq!(changed.notes[0].id, first.notes[0].id);
    }
}
