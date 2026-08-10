/* ============================================================
   Friends / Social — Mock data & helpers
   Squad (clan) + Friends (individual) as separate concepts
   ============================================================ */
(function() {

  const PLAYER = {
    id: 'p-self',
    displayName: 'Student XKCD',
    friendCode: 'XKCD-4921',
    syncStatus: 'ready',
    lastSynced: '2 min ago',
  };

  /* ---- Squad (clan) ---- */
  const SQUAD = {
    name: 'Study Beasts',
    tag: 'SB',
    memberCount: 4,
    rank: 12,        // squad global rank
    totalMinutes: 1840,
    members: [
      { id: 'p-self', displayName: 'Student XKCD', friendCode: 'XKCD-4921', role: 'Leader', sessions: 9, minutes: 389, isSelf: true },
      { id: 'sq-01', displayName: 'FocusPhoenix',  friendCode: 'FPHX-2190', role: 'Member', sessions: 12, minutes: 540 },
      { id: 'sq-02', displayName: 'NightOwlCS',    friendCode: 'NOCS-5512', role: 'Member', sessions: 11, minutes: 495 },
      { id: 'sq-03', displayName: 'QuantumLeap',   friendCode: 'QLEP-8837', role: 'Co-Leader', sessions: 7, minutes: 275 },
    ],
  };

  /* ---- Friends (individual) ---- */
  const FRIENDS_LIST = [
    { id: 'f-01', displayName: 'MathWiz42',     friendCode: 'MWIZ-7834', sessions: 8, minutes: 360 },
    { id: 'f-02', displayName: 'FocusPhoenix',  friendCode: 'FPHX-2190', sessions: 12, minutes: 540 },
    { id: 'f-03', displayName: 'NightOwlCS',    friendCode: 'NOCS-5512', sessions: 11, minutes: 495 },
    { id: 'f-04', displayName: 'StudyStorm',    friendCode: 'SSTM-3346', sessions: 6, minutes: 240 },
    { id: 'f-05', displayName: 'DeepDiver',     friendCode: 'DDVR-6601', sessions: 5, minutes: 190 },
    { id: 'f-06', displayName: 'CodeNinja',     friendCode: 'CNJA-9922', sessions: 8, minutes: 310 },
  ];

  /* ---- Leaderboard generator ---- */
  function makeLeaderboard(scope, period) {
    const globalPlayers = [
      { id: 'g-01', displayName: 'GrindMaster',   friendCode: 'GMAS-1001', sessions: 14, minutes: 680 },
      { id: 'g-02', displayName: 'FocusPhoenix',  friendCode: 'FPHX-2190', sessions: 12, minutes: 540 },
      { id: 'g-03', displayName: 'NightOwlCS',    friendCode: 'NOCS-5512', sessions: 11, minutes: 495 },
      { id: 'p-self', displayName: 'Student XKCD', friendCode: 'XKCD-4921', sessions: 9, minutes: 389, isSelf: true },
      { id: 'g-04', displayName: 'MathWiz42',     friendCode: 'MWIZ-7834', sessions: 8, minutes: 360 },
      { id: 'g-05', displayName: 'CodeNinja',     friendCode: 'CNJA-9922', sessions: 8, minutes: 310 },
      { id: 'g-06', displayName: 'QuantumLeap',   friendCode: 'QLEP-8837', sessions: 7, minutes: 275 },
      { id: 'g-07', displayName: 'StudyStorm',    friendCode: 'SSTM-3346', sessions: 6, minutes: 240 },
      { id: 'g-08', displayName: 'DeepDiver',     friendCode: 'DDVR-6601', sessions: 5, minutes: 190 },
      { id: 'g-09', displayName: 'ByteRunner',    friendCode: 'BRNN-4455', sessions: 4, minutes: 155 },
      { id: 'g-10', displayName: 'ZenCoder',      friendCode: 'ZCDR-7780', sessions: 3, minutes: 95 },
    ];

    let list;
    if (scope === 'global') {
      list = [...globalPlayers];
    } else if (scope === 'friends') {
      list = [
        { id: 'p-self', displayName: 'Student XKCD', friendCode: 'XKCD-4921', sessions: 9, minutes: 389, isSelf: true },
        ...FRIENDS_LIST.map(f => ({ ...f })),
      ];
    } else {
      // squad
      list = SQUAD.members.map(m => ({ ...m }));
    }

    if (period === 'daily') {
      list = list.map(p => ({
        ...p,
        sessions: Math.max(1, Math.floor(p.sessions / 5)),
        minutes: Math.max(10, Math.floor(p.minutes / 6) + (p.isSelf ? 77 : 0)),
      }));
    } else if (period === 'weekly') {
      list = list.map(p => ({
        ...p,
        sessions: Math.max(1, Math.floor(p.sessions / 1.5)),
        minutes: Math.max(20, Math.floor(p.minutes / 2)),
      }));
    }

    list.sort((a, b) => b.minutes - a.minutes);
    return list.map((p, i) => ({ ...p, rank: i + 1 }));
  }

  /* ---- Requests ---- */
  const FRIEND_REQUESTS_IN = [
    { id: 'req-01', displayName: 'AlphaStudy', friendCode: 'ASTY-0088', message: 'Let\'s compete!' },
    { id: 'req-02', displayName: 'FlashCards99', friendCode: 'FC99-1234', message: 'Add me!' },
  ];

  const FRIEND_REQUESTS_OUT = [
    { id: 'req-03', displayName: 'ProtoHacker', friendCode: 'PHKR-5567', sentAt: '3 hours ago' },
  ];

  const SQUAD_INVITES_IN = [
    { id: 'sinv-01', squadName: 'Nerd Herd', tag: 'NH', memberCount: 6, invitedBy: 'BrainStorm' },
  ];

  const LOCAL_STATS = {
    today:   { sessions: 2, minutes: 77 },
    week:    { sessions: 9, minutes: 389 },
    overall: { sessions: 84, minutes: 4210 },
  };

  window.FriendsData = {
    PLAYER,
    SQUAD,
    FRIENDS_LIST,
    FRIEND_REQUESTS_IN,
    FRIEND_REQUESTS_OUT,
    SQUAD_INVITES_IN,
    LOCAL_STATS,
    makeLeaderboard,
  };

})();
