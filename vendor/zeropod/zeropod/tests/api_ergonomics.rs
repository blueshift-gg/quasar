use zeropod::{ZeroPod, ZeroPodCompact, ZeroPodFixed};

// ============================================================
// 1. Fixed mode token account
// ============================================================

#[allow(dead_code)]
#[derive(ZeroPod)]
struct TokenAccount {
    pub mint: [u8; 32],
    pub owner: [u8; 32],
    pub amount: u64,
    pub delegate: Option<[u8; 32]>,
    pub is_frozen: bool,
}

#[test]
fn token_account_fixed() {
    let size = <TokenAccount as ZeroPodFixed>::SIZE;
    // 32 + 32 + 8 + (1+32) + 1 = 106
    assert_eq!(size, 106);

    let mut buf = vec![0u8; size];
    let mint = [1u8; 32];
    let owner = [2u8; 32];
    let delegate = [3u8; 32];

    let acc = TokenAccount::from_bytes_mut(&mut buf).unwrap();
    acc.mint = mint;
    acc.owner = owner;
    acc.amount = 1_000_000u64.into();
    acc.delegate.set(Some(delegate));
    acc.is_frozen = false.into();

    // Read it back
    let acc = TokenAccount::from_bytes(&buf).unwrap();
    assert_eq!(acc.mint, [1u8; 32]);
    assert_eq!(acc.owner, [2u8; 32]);
    assert_eq!(acc.amount.get(), 1_000_000);
    assert_eq!(acc.delegate.get(), Some([3u8; 32]));
    assert!(!acc.is_frozen.get());
}

// ============================================================
// 2. Fixed mode with collections
// ============================================================

#[allow(dead_code)]
#[derive(ZeroPod)]
struct PlayerState {
    pub authority: [u8; 32],
    pub score: u64,
    pub player_name: zeropod::String<16>,
    pub inventory: zeropod::Vec<u8, 20>,
}

#[test]
fn player_state_with_collections() {
    let size = <PlayerState as ZeroPodFixed>::SIZE;
    // 32 + 8 + (1+16) + (2+20) = 79
    assert_eq!(size, 79);

    let mut buf = vec![0u8; size];
    let state = PlayerState::from_bytes_mut(&mut buf).unwrap();
    state.authority = [0xAA; 32];
    state.score = 9999u64.into();
    let _ = state.player_name.set("hero");
    let _ = state.inventory.push(10);
    let _ = state.inventory.push(20);
    let _ = state.inventory.push(30);

    let state = PlayerState::from_bytes(&buf).unwrap();
    assert_eq!(state.authority, [0xAA; 32]);
    assert_eq!(state.score.get(), 9999);
    assert_eq!(state.player_name.as_str(), "hero");
    assert_eq!(state.inventory.as_slice(), &[10, 20, 30]);
}

// ============================================================
// 3. Nested composites
// ============================================================

#[allow(dead_code)]
#[derive(ZeroPod)]
struct Settings {
    pub max_players: u64,
    pub entry_fee: u64,
    pub enabled: bool,
}

#[allow(dead_code)]
#[derive(ZeroPod)]
struct Arena {
    pub authority: [u8; 32],
    pub settings: Settings,
    pub round: u64,
}

#[test]
fn nested_composites() {
    let size = <Arena as ZeroPodFixed>::SIZE;
    // Settings: 8 + 8 + 1 = 17
    // Arena: 32 + 17 + 8 = 57
    assert_eq!(<Settings as ZeroPodFixed>::SIZE, 17);
    assert_eq!(size, 57);

    let mut buf = vec![0u8; size];
    let arena = Arena::from_bytes_mut(&mut buf).unwrap();
    arena.authority = [0xFF; 32];
    arena.settings.max_players = 16u64.into();
    arena.settings.entry_fee = 100u64.into();
    arena.settings.enabled = true.into();
    arena.round = 5u64.into();

    let arena = Arena::from_bytes(&buf).unwrap();
    assert_eq!(arena.authority, [0xFF; 32]);
    assert_eq!(arena.settings.max_players.get(), 16);
    assert_eq!(arena.settings.entry_fee.get(), 100);
    assert!(arena.settings.enabled.get());
    assert_eq!(arena.round.get(), 5);
}

// ============================================================
// 4. Compact mode profile
// ============================================================

#[allow(dead_code)]
#[derive(ZeroPod)]
#[zeropod(compact)]
struct UserProfile {
    pub authority: [u8; 32],
    pub level: u64,
    pub bio: zeropod::String<128>,
    pub tags: zeropod::Vec<[u8; 8], 10>,
}

#[test]
fn compact_profile_ergonomics() {
    let header_size = <UserProfile as ZeroPodCompact>::HEADER_SIZE;
    // authority(32) + PodU64(8) + bio_len(1) + tags_len(2) = 43
    assert_eq!(header_size, 43);

    let mut buf = vec![0u8; 300];
    let auth = [0xBB; 32];
    let tag1 = [1u8; 8];
    let tag2 = [2u8; 8];

    // Write
    {
        let mut profile = UserProfileMut::new(&mut buf).unwrap();
        profile.authority = auth;
        profile.level = 42u64.into();
        profile.set_bio("Solana developer").unwrap();
        let tags = [tag1, tag2];
        profile.set_tags(&tags).unwrap();
        profile.commit().unwrap();
    }

    // Read
    let profile = UserProfileRef::new(&buf).unwrap();
    assert_eq!(profile.authority, [0xBB; 32]);
    assert_eq!(profile.level.get(), 42);
    assert_eq!(profile.bio(), "Solana developer");
    assert_eq!(profile.tags().len(), 2);
    assert_eq!(profile.tags()[0], [1u8; 8]);
    assert_eq!(profile.tags()[1], [2u8; 8]);
}

// ============================================================
// 5. Enum in struct
// ============================================================

#[derive(ZeroPod, Debug, PartialEq)]
#[repr(u8)]
enum GameStatus {
    Waiting = 0,
    Active = 1,
    Finished = 2,
}

#[allow(dead_code)]
#[derive(ZeroPod)]
struct Game {
    pub authority: [u8; 32],
    pub status: GameStatus,
    pub round: u64,
}

#[test]
fn enum_in_struct() {
    let size = <Game as ZeroPodFixed>::SIZE;
    // 32 + 1 (enum u8) + 8 = 41
    assert_eq!(size, 41);

    let mut buf = vec![0u8; size];
    let game = Game::from_bytes_mut(&mut buf).unwrap();
    game.authority = [0xCC; 32];
    game.status = GameStatus::Active.into();
    game.round = 3u64.into();

    let game = Game::from_bytes(&buf).unwrap();
    assert_eq!(game.authority, [0xCC; 32]);
    assert!(game.status == GameStatus::Active);
    assert_eq!(game.status.try_to_enum().unwrap(), GameStatus::Active);
    assert_eq!(game.round.get(), 3);

    // Validation rejects invalid enum discriminant
    let mut bad_buf = buf;
    bad_buf[32] = 5; // invalid status
    assert!(Game::from_bytes(&bad_buf).is_err());
}
