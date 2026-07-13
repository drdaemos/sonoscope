"""Tests for filename heuristic analysis."""

from __future__ import annotations

import pytest

from sonoscope_analyzer.heuristics import analyze_path


def tag_pairs(path: str) -> set[tuple[str, str]]:
    return {(tag.dimension, tag.value) for tag in analyze_path(path)}


@pytest.mark.parametrize(
    "path, expected",
    [
        ("Drums/Kicks/punchy_kick_909.wav", ("Instrument", "kick")),
        ("Drums/Kicks/punchy_kik.wav", ("Instrument", "kick")),
        ("Drums/BD/deep_bd.wav", ("Instrument", "kick")),
        ("Drums/Bass Drum/acoustic bass drum.wav", ("Instrument", "kick")),
        ("Drums/Snares/snare_001.wav", ("Instrument", "snare")),
        ("Drums/Snares/tight_snr.wav", ("Instrument", "snare")),
        ("Drums/SD/sd_rim.wav", ("Instrument", "snare")),
        ("Hats/open_hat.wav", ("Instrument", "hi-hat")),
        ("Hats/hh_closed.wav", ("Instrument", "hi-hat")),
        ("Hats/hi-hat_909.wav", ("Instrument", "hi-hat")),
        ("Hats/hi hat ride.wav", ("Instrument", "hi-hat")),
        ("Claps/clap_big.wav", ("Instrument", "clap")),
        ("Claps/claps_room.wav", ("Instrument", "clap")),
        ("Perc/perc_top.wav", ("Instrument", "percussion")),
        ("Perc/conga_room.wav", ("Instrument", "percussion")),
        ("Perc/bongo_dry.wav", ("Instrument", "percussion")),
        ("Perc/tom_low.wav", ("Instrument", "percussion")),
        ("Perc/rim_click.wav", ("Instrument", "percussion")),
        ("Bass/bass_sub.wav", ("Instrument", "bass")),
        ("Bass/808_long.wav", ("Instrument", "bass")),
        ("Bass/sub_drop.wav", ("Instrument", "bass")),
        ("Chords/minor_chord.wav", ("Instrument", "chord")),
        ("Chords/stabs_house.wav", ("Instrument", "chord")),
        ("Pads/warm_pad.wav", ("Instrument", "pad")),
        ("Pads/ambient_texture.wav", ("Instrument", "pad")),
        ("Synth/synth_pluck.wav", ("Instrument", "synth")),
        ("Synth/arps_120bpm.wav", ("Instrument", "synth")),
        ("Leads/lead_main.wav", ("Instrument", "lead")),
        ("Leads/melody_Cmaj.wav", ("Instrument", "lead")),
        ("Vocals/vocal_phrase.wav", ("Instrument", "vocal")),
        ("Vocals/vox_chop.wav", ("Instrument", "vocal")),
        ("FX/riser_big.wav", ("Instrument", "fx")),
        ("FX/impact_short.wav", ("Instrument", "fx")),
        ("FX/downlifter.wav", ("Instrument", "fx")),
        ("Foley/field_noise.wav", ("Instrument", "foley")),
        ("Foley/foley_hit.wav", ("Instrument", "foley")),
        ("Loops/drum_loop_120bpm.wav", ("Type", "loop")),
        ("Loops/drum_lp.wav", ("Type", "loop")),
        ("Loops/beat_l_120bpm.wav", ("Type", "loop")),
        ("OneShots/kick_oneshot.wav", ("Type", "one-shot")),
        ("OneShots/kick_one_shot.wav", ("Type", "one-shot")),
        ("OneShots/kick_one-shot.wav", ("Type", "one-shot")),
        ("OneShots/kick_1shot.wav", ("Type", "one-shot")),
        ("Loops/bass_95bpm.wav", ("Tempo", "95")),
        ("Loops/bass_120_bpm.wav", ("Tempo", "120")),
        ("Loops/bass 140 bpm.wav", ("Tempo", "140")),
        ("Keys/pad_Cmaj.wav", ("Key", "C")),
        ("Keys/pad_Cmaj.wav", ("Mode", "major")),
        ("Keys/pad_D#_minor.wav", ("Key", "D#")),
        ("Keys/pad_D#_minor.wav", ("Mode", "minor")),
        ("Keys/pad_Bbmin.wav", ("Key", "A#")),
        ("Keys/pad_Bbmin.wav", ("Mode", "minor")),
        ("Keys/pad_f.wav", ("Key", "F")),
        ("Mixed/loop_bass_Cmaj_124bpm.wav", ("Instrument", "bass")),
        ("Mixed/loop_bass_Cmaj_124bpm.wav", ("Type", "loop")),
        ("Mixed/loop_bass_Cmaj_124bpm.wav", ("Tempo", "124")),
        ("Mixed/loop_bass_Cmaj_124bpm.wav", ("Key", "C")),
        ("Mixed/loop_bass_Cmaj_124bpm.wav", ("Mode", "major")),
    ],
)
def test_heuristic_path_tags(path: str, expected: tuple[str, str]) -> None:
    assert expected in tag_pairs(path)


@pytest.mark.parametrize(
    "path, unexpected",
    [
        ("Documents/package.wav", ("Instrument", "kick")),
        ("Drums/snarepack/readme.wav", ("Instrument", "snare")),
        ("Drums/Kicks/unknown.wav", ("Instrument", "kick")),
        ("Loops/unknown.wav", ("Type", "loop")),
        ("Keys/unknown.wav", ("Key", "A")),
        ("Loops/abc120bpmx.wav", ("Tempo", "120")),
        ("Keys/bassline.wav", ("Key", "A")),
        ("Keys/cable.wav", ("Key", "C")),
        # inline tempo: number not preceded by underscore must not match
        ("Studio101_packs/kick.wav", ("Tempo", "101")),
    ],
)
def test_heuristic_avoids_partial_token_matches(path: str, unexpected: tuple[str, str]) -> None:
    assert unexpected not in tag_pairs(path)


@pytest.mark.parametrize(
    "path, expected",
    [
        # --- new type tokens ---
        ("Drums/fills/JAFUNK_102_drum_fill_energy.wav", ("Type", "fill")),
        ("Drums/breaks/BB3_100_drum_break_paprika.wav", ("Type", "break")),
        ("Loops/top_loops/SO_FH_120_top_loop_novadream.wav", ("Type", "loop")),
        ("Loops/top_loops/SO_FH_120_top_loop_novadream.wav", ("Instrument", "tops")),
        ("Loops/drum_top/TS_VK_105_drum_tops_clear_the_way.wav", ("Type", "loop")),
        ("Loops/drum_top/TS_VK_105_drum_tops_clear_the_way.wav", ("Instrument", "tops")),
        ("Textures/EF_texture_fever_dream_120.wav", ("Type", "texture")),
        ("Textures/80_RADIOFEEDBACK_DRONE.wav", ("Type", "texture")),
        # --- new instrument tokens ---
        (
            "Loops/guitar/DSC_MNG_114_electric_guitar_loop_freak_full_Ebmaj.wav",
            ("Instrument", "guitar"),
        ),
        ("Loops/acoustic_guitar/song_acoustic_guitar_loop_Gm.wav", ("Instrument", "guitar")),
        ("Loops/piano/OS_AD_95_piano_chords_hieroglyphics_Am.wav", ("Instrument", "piano")),
        (
            "Loops/electric_piano/SO_FH_120_electric_piano_chords_coldcat_Bmin.wav",
            ("Instrument", "piano"),
        ),
        ("Loops/organ/WW_SB_88_keys_hammond_organ_loop_Fm.wav", ("Instrument", "piano")),
        ("Loops/mellotron/OS_VV2_80_mellotron_chords_storm_Bm.wav", ("Instrument", "piano")),
        ("Loops/brass/JAFUNK_118_brass_section_energy_Bmin.wav", ("Instrument", "brass")),
        ("Loops/trumpet/BO_FAF_90_Horns_Trumpet_Loop_First_Rhythm.wav", ("Instrument", "brass")),
        ("Loops/saxophone/jh_saxophone_loop_moon_80_D.wav", ("Instrument", "woodwind")),
        ("Loops/flute/OS_VV2_70_flute_melody_velvet_Em.wav", ("Instrument", "woodwind")),
        ("Loops/strings/string_loop_Am.wav", ("Instrument", "strings")),
        ("Loops/cymbals/BFTF_-_Crash_Tape_Funk_07.wav", ("Instrument", "cymbal")),
        ("Loops/ride/BFTF_-_Ride_Tape_Funk_02.wav", ("Instrument", "cymbal")),
        # --- full drum kit tokens ---
        ("Loops/JAFUNK_120_drum_loop_thump_bottoms.wav", ("Instrument", "drums")),
        ("Loops/JAFUNK_120_drum_loop_thump_bottoms.wav", ("Type", "loop")),
        ("Drums/kits/drum_kit_groove_98.wav", ("Instrument", "drums")),
        ("Drums/full/live_drums_take3.wav", ("Instrument", "drums")),
        ("Breaks/amen_breakbeat_170.wav", ("Instrument", "drums")),
        ("Drums/breaks/BB3_100_drum_break_paprika.wav", ("Instrument", "drums")),
        # --- expanded percussion tokens ---
        ("Perc/shaker_loop.wav", ("Instrument", "percussion")),
        ("Perc/tambourine_hit.wav", ("Instrument", "percussion")),
        ("Perc/cowbell_120.wav", ("Instrument", "percussion")),
        ("Perc/djembe_slap.wav", ("Instrument", "percussion")),
        # --- expanded clap tokens ---
        ("Claps/KSHMR_Snap_07.wav", ("Instrument", "clap")),
        ("Claps/stomp_hit.wav", ("Instrument", "clap")),
        # --- expanded vocal tokens ---
        ("Vocals/DSC_GDS_100_choir_loop_do_that_praise_dry.wav", ("Instrument", "vocal")),
        ("Vocals/adlib_oh_yeah.wav", ("Instrument", "vocal")),
        ("Vocals/vocal_chops_loop.wav", ("Instrument", "vocal")),
        # --- expanded bass tokens ---
        ("Bass/DSC_EDB_100_electric_bass_loop_precision_Cmin.wav", ("Instrument", "bass")),
        ("Bass/upright_bass_loop_Am.wav", ("Instrument", "bass")),
        # --- expanded fx tokens ---
        ("FX/TL_TFX_104_Uplifter_One_Shot_FX.wav", ("Instrument", "fx")),
        ("FX/TL_TFX_Downlift_OneShot_Growl.wav", ("Instrument", "fx")),
        ("FX/TL_TFX_Atmosphere_One_Shot.wav", ("Instrument", "fx")),
        # --- key: short 'm' minor suffix ---
        ("Keys/pad_Am.wav", ("Key", "A")),
        ("Keys/pad_Am.wav", ("Mode", "minor")),
        ("Keys/pad_Gm.wav", ("Key", "G")),
        ("Keys/pad_Gm.wav", ("Mode", "minor")),
        ("Keys/pad_D#m.wav", ("Key", "D#")),
        ("Keys/pad_D#m.wav", ("Mode", "minor")),
        ("Keys/pad_F#m.wav", ("Key", "F#")),
        ("Keys/pad_F#m.wav", ("Mode", "minor")),
        ("Keys/pad_Ebm.wav", ("Key", "D#")),
        ("Keys/pad_Ebm.wav", ("Mode", "minor")),
        # --- inline tempo (Splice _NNN_ convention, no 'bpm') ---
        ("packs/JAFUNK_120_drum_loop_thump_bottoms.wav", ("Tempo", "120")),
        ("packs/OS_LFC_130_drum_loop_zest_alt.wav", ("Tempo", "130")),
        ("packs/PHOTEK_drum_loop_bambu_175.wav", ("Tempo", "175")),
        ("packs/KSHMR_Shaker_Loop_18_128.wav", ("Tempo", "128")),
    ],
)
def test_heuristic_new_tokens(path: str, expected: tuple[str, str]) -> None:
    assert expected in tag_pairs(path)


def test_heuristics_do_not_default_one_shot_without_type_evidence() -> None:
    assert ("Type", "one-shot") not in tag_pairs("Drums/Snares/snare_001.wav")
