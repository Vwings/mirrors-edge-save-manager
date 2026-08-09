# Mirror's Edge Save Format Research

Status: Paused

This document records only the stable conclusions needed to preserve future
research context. It is not a format specification. Production capture,
storage, Apply, and recovery code must continue to treat save bytes as opaque.

Untracked research inputs are kept under `scratch/save-format/samples/` and must
never be overwritten by automated tooling.

## Physical Layout

Observed PC saves are fixed-size containers of `9,134,256` bytes (`0x8B60B0`):

```text
0x00000000 - 0x0001869F   descriptor/header area (100,000 bytes)
0x000186A0 - 0x008ADADF   30 slots, 300,000 bytes each
0x008ADAE0 - 0x008B500F   3 slots, 10,000 bytes each
0x008B5010 - 0x008B5FAF   1 Profile slot, 4,000 bytes
0x008B5FB0 - 0x008B60AF   1 slot, 256 bytes
```

The large-slot descriptors expose slot identity, capacity, occupancy, declared
used length, and a 16-byte digest. Bytes after a declared used length are not
authoritative and may contain stale data.

## Confirmed Integrity

- Occupied 300,000-byte slots use descriptor MD5 over their declared payload.
- The Profile begins with SHA-1 over the remaining declared Profile payload.
- The Profile descriptor uses MD5 over the complete declared Profile payload.
- A separate 16-byte field near header offset `0x57C` changes with Profile
  content. Its algorithm and coverage remain unknown.

The unidentified field is a blocker for safe offline editing. It must not be
copied from another content state or ignored without game validation.

## Known Semantics

- Large occupied slots contain zlib-compressed Ghost/Replay-like data.
- The Profile slot contains progression, unlock masks, result records, and
  other compact player settings.
- Profile records are variable-length; later records move when an earlier value
  changes representation.
- Time Trial unlock state, qualifier state, PB results, and Ghost data are
  separate concepts in the game API.
- The Time Trial unlock mask is setting ID `1150`; all 24 bits set is
  `0x00FFFFFF`.
- A controlled game-generated sample changed the unlock mask without creating
  PB result records or Ghost slots.
- Reapplying the same all-unlocked value produced a byte-identical save.
- Writing zero produced a shorter Profile than either a normal partial mask or
  the all-unlocked mask, consistent with compact/default-value serialization.

The compact result table contains 33 entries from `B1` through `D1`. The counts
support, but do not yet prove, this mapping:

- `B1..C7`: 23 base-game Time Trials.
- `C8..D0`: 9 Pure Time Trials DLC courses.
- `D1`: the PS3-exclusive Synesthesia definition retained in shared data.

The preceding `B0` entry and the relationship between the 24-bit unlock mask
and the 33 result entries remain unresolved.

## Community Tool Evidence

MirrorsEdgeTweaks does not implement a `.dat` parser. Its companion UnrealScript
package edits progress through the game's `TdProfileSettings`, Time Trial, and
Ghost APIs, then asks the game to serialize the save.

Relevant operations confirm that the game can independently:

- Set story and Time Trial unlock masks.
- Set collected-bag and viewed-hint state.
- Reset Time Trial PB data and Ghosts.
- Reset speedrun PB data.
- Read and write Time Trial stretch data.

This evidence identifies business concepts and setting IDs, but does not define
the on-disk serializer or integrity scheme.

## External Configuration

PC input and graphics configuration also exists outside the save file:

- `TdInput.ini` stores mouse sensitivity and key bindings.
- `TdEngine.ini` stores display gamma.

Some preferences may still be mirrored through the Profile when the game loads
or saves. A future investigation should compare both INI files while replacing
only the `.dat` before assigning ownership to either storage location.

## Unresolved Work

An offline editor would still require:

1. A complete Profile record framing and compact-value decoder.
2. Stable setting-ID and Time Trial stretch mappings.
3. The `0x57C` integrity algorithm or proof that the game ignores it.
4. Ghost header, payload, and slot-association semantics for Ghost editing.
5. Version checks across supported PC releases.
6. Byte-exact parse/serialize round trips and in-game validation.

This work is intentionally paused. It should resume only for a concrete editing
feature whose user value justifies the additional reverse engineering and
validation burden.
