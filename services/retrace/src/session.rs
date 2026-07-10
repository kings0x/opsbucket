use std::collections::HashSet;

pub struct SessionBuffer {
    seen_chunks: HashSet<(String, u32)>,
}

impl SessionBuffer {
    pub fn new() -> Self {
        Self {
            seen_chunks: HashSet::new(),
        }
    }

    pub fn is_duplicate(&self, session_id: &str, chunk_seq: u32) -> bool {
        self.seen_chunks
            .contains(&(session_id.to_string(), chunk_seq))
    }

    pub fn mark_seen(&mut self, session_id: &str, chunk_seq: u32) {
        self.seen_chunks.insert((session_id.to_string(), chunk_seq));
    }

    pub fn evict_session(&mut self, session_id: &str) {
        self.seen_chunks.retain(|(sid, _)| sid != session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_duplicate_chunk() {
        let mut buf = SessionBuffer::new();
        assert!(!buf.is_duplicate("sess-1", 0));
        buf.mark_seen("sess-1", 0);
        assert!(buf.is_duplicate("sess-1", 0));
    }

    #[test]
    fn different_chunk_seqs_are_not_duplicates() {
        let mut buf = SessionBuffer::new();
        buf.mark_seen("sess-1", 0);
        assert!(!buf.is_duplicate("sess-1", 1));
    }

    #[test]
    fn different_sessions_are_independent() {
        let mut buf = SessionBuffer::new();
        buf.mark_seen("sess-a", 0);
        assert!(!buf.is_duplicate("sess-b", 0));
    }

    #[test]
    fn evict_removes_all_chunks_for_session() {
        let mut buf = SessionBuffer::new();
        buf.mark_seen("sess-1", 0);
        buf.mark_seen("sess-1", 1);
        buf.mark_seen("sess-2", 0);
        buf.evict_session("sess-1");
        assert!(!buf.is_duplicate("sess-1", 0));
        assert!(buf.is_duplicate("sess-2", 0));
    }

    #[test]
    fn unseen_session_returns_false() {
        let buf = SessionBuffer::new();
        assert!(!buf.is_duplicate("unknown-session", 42));
    }

    #[test]
    fn evict_nonexistent_session_does_not_panic() {
        let mut buf = SessionBuffer::new();
        buf.mark_seen("sess-1", 0);
        buf.evict_session("nonexistent");
        assert!(buf.is_duplicate("sess-1", 0));
    }

    #[test]
    fn many_chunk_seqs_for_same_session() {
        let mut buf = SessionBuffer::new();
        for i in 0..1000 {
            assert!(!buf.is_duplicate("sess-1", i));
            buf.mark_seen("sess-1", i);
        }
        for i in 0..1000 {
            assert!(buf.is_duplicate("sess-1", i));
        }
    }
}
