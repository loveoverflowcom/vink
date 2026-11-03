//! EMO-INV-01..05. A single mutable owner admits every player in its host.
//! Reservation is the Loading/Playing phase itself, so releasing twice cannot underflow.

pub const MAX_ACTIVE: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Static,
    Loading,
    Playing,
    Error,
    Disposed,
}

impl Phase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Loading => "loading",
            Self::Playing => "playing",
            Self::Error => "error",
            Self::Disposed => "disposed",
        }
    }

    pub const fn active(self) -> bool {
        matches!(self, Self::Loading | Self::Playing)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    Idle,
    ReducedMotion,
    Background,
    Offscreen,
    Unsupported,
    Budget,
    Complete,
    Cancelled,
    Failed,
    Disposed,
    GenerationExhausted,
}

impl Reason {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "Phát một lần",
            Self::ReducedMotion => "Hình tĩnh · giảm chuyển động",
            Self::Background => "Hình tĩnh · tab đang ẩn",
            Self::Offscreen => "Hình tĩnh · ngoài vùng nhìn",
            Self::Unsupported => "Hình tĩnh · chưa hỗ trợ chuyển động",
            Self::Budget => "Đang đủ lượt phát · thử lại sau",
            Self::Complete => "Đã phát · phát lại một lần",
            Self::Cancelled => "Đã dừng · phát lại một lần",
            Self::Failed => "Chưa tải được chuyển động · thử lại",
            Self::Disposed | Self::GenerationExhausted => "Đã đóng",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admission {
    Start(u32),
    AlreadyActive(u32),
    Static(Reason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Player {
    phase: Phase,
    generation: u32,
    visible: bool,
    animated: bool,
    reason: Reason,
}

impl Player {
    pub const fn phase(self) -> Phase {
        self.phase
    }
    pub const fn generation(self) -> u32 {
        self.generation
    }
    pub const fn reason(self) -> Reason {
        self.reason
    }
    pub const fn label(self) -> &'static str {
        match self.phase {
            Phase::Loading => "Đang tải chuyển động…",
            Phase::Playing => "Đang phát một lần",
            _ => self.reason.label(),
        }
    }
}

/// One controller per mounted host, shared by all its cells. Indices are mount-local,
/// never persisted asset references. Disposed indices are never reused in this controller.
#[derive(Clone, Debug)]
pub struct Policy<const N: usize> {
    players: [Player; N],
    capacity: usize,
    reduced_motion: bool,
    foreground: bool,
    runtime_supported: bool,
}

impl<const N: usize> Policy<N> {
    pub fn new(capacity: usize, animated: [bool; N]) -> Self {
        Self {
            players: animated.map(|animated| Player {
                phase: Phase::Static,
                generation: 0,
                visible: false,
                animated,
                reason: Reason::Idle,
            }),
            capacity: capacity.min(MAX_ACTIVE),
            // A host must supply actual facts before activation is eligible.
            reduced_motion: true,
            foreground: false,
            runtime_supported: false,
        }
    }

    pub fn active(&self) -> usize {
        self.players
            .iter()
            .filter(|player| player.phase.active())
            .count()
    }

    pub const fn capacity(&self) -> usize {
        self.capacity
    }
    pub fn player(&self, index: usize) -> Option<Player> {
        self.players.get(index).copied()
    }

    fn blocked(&self, index: usize) -> Option<Reason> {
        let player = self.players.get(index)?;
        if player.phase == Phase::Disposed {
            Some(Reason::Disposed)
        } else if !player.animated || !self.runtime_supported {
            Some(Reason::Unsupported)
        } else if self.reduced_motion {
            Some(Reason::ReducedMotion)
        } else if !self.foreground {
            Some(Reason::Background)
        } else if !player.visible {
            Some(Reason::Offscreen)
        } else {
            None
        }
    }

    /// Admission and reservation occur in the same exclusive call, never check-then-reserve.
    pub fn activate(&mut self, index: usize) -> Admission {
        let Some(player) = self.player(index) else {
            return Admission::Static(Reason::Disposed);
        };
        if let Some(reason) = self.blocked(index) {
            if player.phase != Phase::Disposed {
                self.players[index].reason = reason;
            }
            return Admission::Static(reason);
        }
        if player.phase.active() {
            return Admission::AlreadyActive(player.generation);
        }
        if self.active() >= self.capacity {
            self.players[index].reason = Reason::Budget;
            return Admission::Static(Reason::Budget);
        }
        let Some(generation) = player.generation.checked_add(1) else {
            self.players[index].phase = Phase::Disposed;
            self.players[index].reason = Reason::GenerationExhausted;
            return Admission::Static(Reason::GenerationExhausted);
        };
        self.players[index].generation = generation;
        self.players[index].phase = Phase::Loading;
        self.players[index].reason = Reason::Idle;
        Admission::Start(generation)
    }

    /// A load callback is only authority to play its own still-admitted generation.
    pub fn ready(&mut self, index: usize, generation: u32) -> bool {
        if self.blocked(index).is_some() {
            return false;
        }
        let Some(player) = self.players.get_mut(index) else {
            return false;
        };
        if player.phase != Phase::Loading || player.generation != generation {
            return false;
        }
        player.phase = Phase::Playing;
        true
    }

    pub fn release(&mut self, index: usize, generation: u32) -> bool {
        self.end(index, generation, Phase::Static, Reason::Complete)
    }

    pub fn fail(&mut self, index: usize, generation: u32) -> bool {
        self.end(index, generation, Phase::Error, Reason::Failed)
    }

    fn end(&mut self, index: usize, generation: u32, phase: Phase, reason: Reason) -> bool {
        let Some(player) = self.players.get_mut(index) else {
            return false;
        };
        if !player.phase.active() || player.generation != generation {
            return false;
        }
        player.phase = phase;
        player.reason = reason;
        true
    }

    pub fn cancel(&mut self, index: usize) {
        if let Some(player) = self.players.get_mut(index)
            && player.phase.active()
        {
            player.phase = Phase::Static;
            player.reason = Reason::Cancelled;
        }
    }

    pub fn dispose(&mut self, index: usize) {
        if let Some(player) = self.players.get_mut(index) {
            player.phase = Phase::Disposed;
            player.reason = Reason::Disposed;
        }
    }

    pub fn set_visible(&mut self, index: usize, visible: bool) {
        let Some(player) = self.players.get_mut(index) else {
            return;
        };
        if player.visible == visible {
            return;
        }
        player.visible = visible;
        self.reconcile();
    }

    pub fn set_environment(&mut self, reduced_motion: bool, foreground: bool, runtime: bool) {
        if (self.reduced_motion, self.foreground, self.runtime_supported)
            == (reduced_motion, foreground, runtime)
        {
            return;
        }
        self.reduced_motion = reduced_motion;
        self.foreground = foreground;
        self.runtime_supported = runtime;
        self.reconcile();
    }

    fn reconcile(&mut self) {
        for index in 0..N {
            if let Some(reason) = self.blocked(index) {
                if self.players[index].phase != Phase::Disposed {
                    self.players[index].phase = Phase::Static;
                    self.players[index].reason = reason;
                }
            } else if !self.players[index].phase.active()
                && self.players[index].phase != Phase::Error
            {
                self.players[index].reason = Reason::Idle;
            }
        }
    }
}

#[cfg(test)]
mod overflow_tests {
    use super::*;
    #[test]
    fn generation_exhaustion_is_terminal_instead_of_wrapping() {
        let mut policy = Policy::new(1, [true]);
        policy.set_environment(false, true, true);
        policy.set_visible(0, true);
        policy.players[0].generation = u32::MAX;
        assert_eq!(
            policy.activate(0),
            Admission::Static(Reason::GenerationExhausted)
        );
        assert!(!policy.ready(0, 0));
        assert!(!policy.ready(0, u32::MAX));
        assert_eq!(policy.player(0).unwrap().phase(), Phase::Disposed);
        assert_eq!(policy.active(), 0);
    }
}
