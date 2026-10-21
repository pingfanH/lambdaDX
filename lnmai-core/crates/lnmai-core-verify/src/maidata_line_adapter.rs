//! Concrete Maidata line dictionaries shared by extraction and the runtime.
//!
//! Indexed reads, header field access, chart construction/update and both Vec
//! pushes live here. The caller supplies only operations on one text or view;
//! no primitive receives a line collection, cursor, chart, or scan result.

use crate::maidata_scan::{LineRead, LineWrite};

/// Same field order, derives and struct name as the former runtime record.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MaidataChartBlock<Text, Level> {
    pub level_index: Level,
    pub raw_body: Text,
}

/// Single-text boundary. Views borrow from the supplied text/view, not from
/// this primitive dictionary. Runtime uses Text = String, View = str, and
/// Level = Natural. Borrowing and trimming allocate nothing.
///
/// `trim` removes only U+0020, U+0009, U+000A and U+000D. `header` retains the
/// existing first-'=' helper, including its key-before-value allocations.
/// `level` retains strip_prefix("&inote_")/Natural parsing/default on one key.
/// These are explicit primitive contracts, not claims about arbitrary Ops.
pub trait LinePrimitives<Text> {
    type View: ?Sized;
    type Level;

    fn borrow<'a>(&self, text: &'a Text) -> &'a Self::View;
    fn trim<'a>(&self, view: &'a Self::View) -> &'a Self::View;
    fn is_empty(&self, view: &Self::View) -> bool;
    fn starts_with_amp(&self, view: &Self::View) -> bool;
    fn starts_with_inote(&self, view: &Self::View) -> bool;
    fn header(&self, view: &Self::View) -> Option<(Text, Text)>;
    fn level(&self, view: &Self::View) -> Self::Level;
    fn push_newline(&self, text: &mut Text);
    fn push_str(&self, text: &mut Text, view: &Self::View);
}

/// The source slice stays borrowed; no line or existing output is cloned.
pub struct MaidataLineOps<'a, Text, Ops> {
    pub lines: &'a [Text],
    pub primitives: Ops,
}

impl<Text, Ops> LineRead<(Text, Text)> for MaidataLineOps<'_, Text, Ops>
where
    Ops: LinePrimitives<Text>,
{
    fn is_blank(&self, index: usize) -> bool {
        let view = self.primitives.borrow(&self.lines[index]);
        self.primitives.is_empty(self.primitives.trim(view))
    }

    fn starts_with_amp(&self, index: usize) -> bool {
        let view = self.primitives.borrow(&self.lines[index]);
        self.primitives.starts_with_amp(self.primitives.trim(view))
    }

    fn header(&self, index: usize) -> Option<(Text, Text)> {
        // Split the original line; do not trim the whole line first.
        self.primitives
            .header(self.primitives.borrow(&self.lines[index]))
    }

    fn is_chart_header(&self, header: &(Text, Text)) -> bool {
        self.primitives
            .starts_with_inote(self.primitives.borrow(&header.0))
    }
}

impl<Text, Ops>
    LineWrite<
        (Text, Text),
        Vec<(Text, Text)>,
        Vec<MaidataChartBlock<Text, Ops::Level>>,
        MaidataChartBlock<Text, Ops::Level>,
    > for MaidataLineOps<'_, Text, Ops>
where
    Ops: LinePrimitives<Text>,
{
    fn append_field(
        &self,
        mut fields: Vec<(Text, Text)>,
        header: (Text, Text),
    ) -> Vec<(Text, Text)> {
        fields.push(header);
        fields
    }

    fn begin_chart(&self, header: (Text, Text)) -> MaidataChartBlock<Text, Ops::Level> {
        let (key, value) = header;
        let level_index = self.primitives.level(self.primitives.borrow(&key));
        MaidataChartBlock {
            level_index,
            raw_body: value,
        }
    }

    fn append_body_line(
        &self,
        mut body: MaidataChartBlock<Text, Ops::Level>,
        index: usize,
    ) -> MaidataChartBlock<Text, Ops::Level> {
        // Keep newline mutation/allocation before even indexing the input.
        self.primitives.push_newline(&mut body.raw_body);
        self.primitives.push_str(
            &mut body.raw_body,
            self.primitives.borrow(&self.lines[index]),
        );
        body
    }

    fn append_chart(
        &self,
        mut charts: Vec<MaidataChartBlock<Text, Ops::Level>>,
        body: MaidataChartBlock<Text, Ops::Level>,
    ) -> Vec<MaidataChartBlock<Text, Ops::Level>> {
        charts.push(body);
        charts
    }
}
