// Group play, free of game types: who is in which group, invitations, and who near an earner gets
// a share of the experience. groups.rs keeps one of these in memory for the server.
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

/// How long an invitation waits for `/pct group accept`.
pub const INVITE_LIFETIME: Duration = Duration::from_secs(120);

/// Why a group command did nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    SelfInvite,
    AlreadyTogether,
    InAnotherGroup,
    Full,
    NoInvite,
    Expired,
    NotInGroup,
}

impl Refusal {
    /// What the player is told, in the game's language.
    pub fn text(&self) -> &'static str {
        match self {
            Refusal::SelfInvite => "Нельзя пригласить себя",
            Refusal::AlreadyTogether => "Этот игрок уже в вашей группе",
            Refusal::InAnotherGroup => "Игрок уже в другой группе",
            Refusal::Full => "В группе нет места",
            Refusal::NoInvite => "Приглашений нет",
            Refusal::Expired => "Приглашение устарело",
            Refusal::NotInGroup => "Вы не в группе",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Invite {
    from: u128,
    at: Instant,
}

/// The server's groups. A group exists while it has two members or more.
#[derive(Debug, Default)]
pub struct Groups {
    next_id: u32,
    groups: BTreeMap<u32, Vec<u128>>,
    member_of: BTreeMap<u128, u32>,
    /// The invitation each player has, by who is invited.
    invites: BTreeMap<u128, Invite>,
}

impl Groups {
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    /// The members of `player`'s group, themselves included, in the order they joined; empty when
    /// they are in none.
    pub fn members(&self, player: u128) -> &[u128] {
        self.member_of
            .get(&player)
            .and_then(|id| self.groups.get(id))
            .map_or(&[], Vec::as_slice)
    }

    /// Who invited `player`, if the invitation still holds at `now`.
    pub fn invited_by(&self, player: u128, now: Instant) -> Option<u128> {
        self.invites
            .get(&player)
            .filter(|invite| now.duration_since(invite.at) < INVITE_LIFETIME)
            .map(|invite| invite.from)
    }

    /// `from` invites `to`; a newer invitation replaces an older one.
    pub fn invite(
        &mut self,
        from: u128,
        to: u128,
        max: usize,
        now: Instant,
    ) -> Result<(), Refusal> {
        if from == to {
            return Err(Refusal::SelfInvite);
        }
        let mine = self.member_of.get(&from);
        if mine.is_some() && mine == self.member_of.get(&to) {
            return Err(Refusal::AlreadyTogether);
        }
        if self.member_of.contains_key(&to) {
            return Err(Refusal::InAnotherGroup);
        }
        if self.members(from).len() >= max {
            return Err(Refusal::Full);
        }
        self.invites.insert(to, Invite { from, at: now });
        Ok(())
    }

    /// `player` takes their invitation: they join the inviter's group, or the two start one. The
    /// members after joining come back.
    pub fn accept(&mut self, player: u128, max: usize, now: Instant) -> Result<Vec<u128>, Refusal> {
        let invite = self.invites.remove(&player).ok_or(Refusal::NoInvite)?;
        if now.duration_since(invite.at) >= INVITE_LIFETIME {
            return Err(Refusal::Expired);
        }
        if self.member_of.contains_key(&player) {
            return Err(Refusal::InAnotherGroup);
        }
        let id = match self.member_of.get(&invite.from) {
            Some(id) => *id,
            None => {
                self.next_id = self.next_id.wrapping_add(1);
                let id = self.next_id;
                self.groups.insert(id, vec![invite.from]);
                self.member_of.insert(invite.from, id);
                id
            }
        };
        let group = self.groups.entry(id).or_default();
        if group.len() >= max {
            return Err(Refusal::Full);
        }
        group.push(player);
        self.member_of.insert(player, id);
        Ok(group.clone())
    }

    /// `player` leaves their group; the members still in it come back. A group left with one
    /// member ends.
    pub fn leave(&mut self, player: u128) -> Result<Vec<u128>, Refusal> {
        let id = self.member_of.remove(&player).ok_or(Refusal::NotInGroup)?;
        let Some(group) = self.groups.get_mut(&id) else {
            return Ok(Vec::new());
        };
        group.retain(|member| *member != player);
        let left = group.clone();
        if left.len() < 2 {
            self.groups.remove(&id);
            for member in &left {
                self.member_of.remove(member);
            }
        }
        Ok(left)
    }

    /// Everything about `player` goes: their place in a group, invitations to and from them.
    pub fn forget(&mut self, player: u128) {
        let _ = self.leave(player);
        self.invites
            .retain(|to, invite| *to != player && invite.from != player);
    }
}

/// An online player as group play sees them.
#[derive(Debug, Clone, PartialEq)]
pub struct Nearby {
    pub uuid: u128,
    pub dimension: String,
    pub position: [f64; 3],
    /// Survival or adventure: creative and spectator players earn nothing.
    pub can_earn: bool,
}

/// The members of the earner's group who get a share: online, able to earn, in the earner's
/// dimension and within `radius` blocks. An earner who is not online gives nobody anything.
pub fn share_recipients(
    earner: u128,
    members: &[u128],
    roster: &[Nearby],
    radius: f64,
) -> Vec<u128> {
    let find = |uuid: u128| roster.iter().find(|p| p.uuid == uuid);
    let Some(from) = find(earner) else {
        return Vec::new();
    };
    members
        .iter()
        .filter(|member| **member != earner)
        .filter_map(|member| find(*member))
        .filter(|p| p.can_earn && p.dimension == from.dimension)
        .filter(|p| distance_squared(p.position, from.position) <= radius * radius)
        .map(|p| p.uuid)
        .collect()
}

fn distance_squared(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum()
}

/// What each recipient gets from `gained`: `share` of it, nothing for nothing.
pub fn share_amount(gained: f64, share: f64) -> f64 {
    if gained.is_finite() && gained > 0.0 && share > 0.0 {
        gained * share
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: u128 = 1;
    const B: u128 = 2;
    const C: u128 = 3;
    const D: u128 = 4;
    const E: u128 = 5;

    fn now() -> Instant {
        Instant::now()
    }

    #[test]
    fn an_accepted_invitation_starts_a_group_and_the_next_joins_it() {
        let mut groups = Groups::default();
        let t = now();
        assert!(groups.members(A).is_empty());
        groups.invite(A, B, 4, t).unwrap();
        assert_eq!(groups.invited_by(B, t), Some(A));
        assert_eq!(groups.accept(B, 4, t).unwrap(), vec![A, B]);
        groups.invite(B, C, 4, t).unwrap();
        assert_eq!(groups.accept(C, 4, t).unwrap(), vec![A, B, C]);
        assert_eq!(groups.members(A), [A, B, C]);
        assert_eq!(groups.members(C), [A, B, C]);
        assert!(!groups.is_empty());
    }

    #[test]
    fn invitations_that_cannot_work_are_refused() {
        let mut groups = Groups::default();
        let t = now();
        assert_eq!(groups.invite(A, A, 4, t), Err(Refusal::SelfInvite));
        groups.invite(A, B, 4, t).unwrap();
        groups.accept(B, 4, t).unwrap();
        assert_eq!(groups.invite(A, B, 4, t), Err(Refusal::AlreadyTogether));
        groups.invite(C, D, 4, t).unwrap();
        groups.accept(D, 4, t).unwrap();
        assert_eq!(groups.invite(A, C, 4, t), Err(Refusal::InAnotherGroup));
        assert_eq!(groups.accept(E, 4, t), Err(Refusal::NoInvite));
    }

    #[test]
    fn a_group_never_grows_past_its_size() {
        let mut groups = Groups::default();
        let t = now();
        groups.invite(A, B, 2, t).unwrap();
        groups.accept(B, 2, t).unwrap();
        assert_eq!(groups.invite(A, C, 2, t), Err(Refusal::Full));
        // Two invitations out at once: only one of them fits.
        let mut groups = Groups::default();
        groups.invite(A, B, 3, t).unwrap();
        groups.invite(A, C, 3, t).unwrap();
        groups.invite(A, D, 3, t).unwrap();
        groups.accept(B, 3, t).unwrap();
        groups.accept(C, 3, t).unwrap();
        assert_eq!(groups.accept(D, 3, t), Err(Refusal::Full));
        assert_eq!(groups.members(A).len(), 3);
    }

    #[test]
    fn an_old_invitation_expires() {
        let mut groups = Groups::default();
        let t = now();
        groups.invite(A, B, 4, t).unwrap();
        let later = t + INVITE_LIFETIME;
        assert_eq!(groups.invited_by(B, later), None);
        assert_eq!(groups.accept(B, 4, later), Err(Refusal::Expired));
        assert!(groups.members(A).is_empty());
    }

    #[test]
    fn leaving_ends_a_group_of_two_and_shrinks_a_bigger_one() {
        let mut groups = Groups::default();
        let t = now();
        for who in [B, C] {
            groups.invite(A, who, 4, t).unwrap();
            groups.accept(who, 4, t).unwrap();
        }
        assert_eq!(groups.leave(A).unwrap(), vec![B, C]);
        assert_eq!(groups.members(B), [B, C]);
        assert_eq!(groups.leave(C).unwrap(), vec![B]);
        assert!(groups.members(B).is_empty());
        assert!(groups.is_empty());
        assert_eq!(groups.leave(B), Err(Refusal::NotInGroup));
    }

    #[test]
    fn a_forgotten_player_leaves_and_their_invitations_go() {
        let mut groups = Groups::default();
        let t = now();
        groups.invite(D, A, 4, t).unwrap();
        groups.invite(A, B, 4, t).unwrap();
        groups.accept(B, 4, t).unwrap();
        groups.invite(A, C, 4, t).unwrap();
        groups.forget(A);
        assert!(groups.members(B).is_empty());
        assert_eq!(groups.invited_by(C, t), None);
        assert_eq!(groups.accept(A, 4, t), Err(Refusal::NoInvite));
    }

    fn at(uuid: u128, x: f64) -> Nearby {
        Nearby {
            uuid,
            dimension: "minecraft:overworld".to_owned(),
            position: [x, 64.0, 0.0],
            can_earn: true,
        }
    }

    #[test]
    fn only_members_near_the_earner_who_can_earn_get_a_share() {
        let far = at(C, 40.0);
        let creative = Nearby {
            can_earn: false,
            ..at(D, 1.0)
        };
        let nether = Nearby {
            dimension: "minecraft:the_nether".to_owned(),
            ..at(E, 1.0)
        };
        let roster = [at(A, 0.0), at(B, 32.0), far, creative, nether];
        let members = [A, B, C, D, E];
        assert_eq!(share_recipients(A, &members, &roster, 32.0), vec![B]);
        // A member who is not online gets nothing, and an earner who is not online shares nothing.
        assert_eq!(share_recipients(A, &[A, B, 9], &roster, 32.0), vec![B]);
        assert!(share_recipients(9, &[9, A, B], &roster, 32.0).is_empty());
        // Somebody near but not in the group gets nothing.
        assert!(share_recipients(A, &[A, C], &roster, 32.0).is_empty());
    }

    #[test]
    fn the_share_is_a_part_of_what_was_gained() {
        assert_eq!(share_amount(10.0, 0.15), 1.5);
        assert_eq!(share_amount(0.0, 0.15), 0.0);
        assert_eq!(share_amount(-3.0, 0.15), 0.0);
        assert_eq!(share_amount(f64::NAN, 0.15), 0.0);
        assert_eq!(share_amount(10.0, 0.0), 0.0);
    }
}
