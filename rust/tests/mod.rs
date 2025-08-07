//! Common test utilities

use syan::span::Span;

#[derive(Clone, Debug, Default)]
pub struct TestSpan;

impl Span for TestSpan {
    fn migrate(self, _other: Self) -> Self {
        TestSpan
    }
}