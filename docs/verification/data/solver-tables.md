## arm uncapped

Files: solver-uncapped-real.json: publishable=True contended=False load 2.93->3.11 runs/side=10; solver-uncapped-synthetic-fast.json: publishable=True contended=False load 3.11->2.86 runs/side=10; solver-uncapped-synthetic-slow.json: publishable=True contended=False load 2.86->3.32 runs/side=10

### synthetic speedup (old median / new median, wall; bootstrap 95% CI)

| constraints | chain | grid | mix | clustered |
|---:|---|---|---|---|
| 50 | 5.20x [4.94, 5.51] | 6.00x [5.57, 6.24] | 10.6x [9.36, 11.2] | 2.57x [2.32, 2.67] |
| 200 | 29.9x [27.5, 31.9] | 32.7x [31.0, 34.0] | 92.4x [87.4, 99.4] | 2.61x [2.54, 2.65] |
| 500 | 128x [126, 135] | 134x [121, 140] | 475x [466, 503] | 2.62x [2.56, 2.66] |
| 1000 | 407x [399, 428] | 419x [404, 432] | 1,682x [1,665, 1,709] | 2.58x [2.53, 2.62] |
| 2000 | 1,219x [1,195, 1,244] | 1,224x [1,202, 1,255] | 5,740x [5,615, 5,808] | 2.46x [2.42, 2.51] |
| 5000 | 3,562x [3,553, 3,636] | 3,328x [3,271, 3,367] | 3,595x [3,523, 3,656] | 2.48x [2.47, 2.54] |

### synthetic absolute medians (ms) old / new, and CPU-time speedup

| input | constraints | old ms | new ms | wall speedup | CPU speedup | clone us | batch |
|---|---:|---:|---:|---|---|---:|---:|
| syn-chain-0050 | 50 | 0.548 | 0.105 | 5.20x [4.94, 5.51] | 5.21x [4.93, 5.47] | 3.2 | 3 |
| syn-chain-0200 | 200 | 4.271 | 0.143 | 29.9x [27.5, 31.9] | 29.8x [27.5, 31.8] | 6.5 | 1 |
| syn-chain-0500 | 500 | 48.583 | 0.380 | 128x [126, 135] | 127x [126, 134] | 14.1 | 1 |
| syn-chain-1000 | 1000 | 344.893 | 0.847 | 407x [399, 428] | 406x [400, 422] | 24.6 | 1 |
| syn-chain-2000 | 2000 | 2586.794 | 2.123 | 1,219x [1,195, 1,244] | 1,223x [1,192, 1,241] | 45.5 | 1 |
| syn-chain-5000 | 5000 | 19926.157 | 5.594 | 3,562x [3,553, 3,636] | 3,564x [3,543, 3,641] | 115.9 | 1 |
| syn-clustered-0050 | 50 | 0.329 | 0.128 | 2.57x [2.32, 2.67] | 2.57x [2.33, 2.67] | 2.7 | 1 |
| syn-clustered-0200 | 200 | 1.317 | 0.505 | 2.61x [2.54, 2.65] | 2.60x [2.55, 2.65] | 4.8 | 1 |
| syn-clustered-0500 | 486 | 3.391 | 1.294 | 2.62x [2.56, 2.66] | 2.62x [2.56, 2.64] | 9.4 | 1 |
| syn-clustered-1000 | 972 | 6.729 | 2.609 | 2.58x [2.53, 2.62] | 2.58x [2.53, 2.61] | 16.3 | 1 |
| syn-clustered-2000 | 1930 | 13.554 | 5.501 | 2.46x [2.42, 2.51] | 2.46x [2.43, 2.50] | 31.1 | 1 |
| syn-clustered-5000 | 4833 | 33.938 | 13.660 | 2.48x [2.47, 2.54] | 2.49x [2.47, 2.54] | 76.0 | 1 |
| syn-grid-0050 | 50 | 0.675 | 0.112 | 6.00x [5.57, 6.24] | 5.98x [5.56, 6.17] | 3.2 | 3 |
| syn-grid-0200 | 200 | 4.869 | 0.149 | 32.7x [31.0, 34.0] | 32.5x [30.8, 33.8] | 6.3 | 1 |
| syn-grid-0500 | 500 | 53.390 | 0.399 | 134x [121, 140] | 134x [129, 140] | 13.5 | 1 |
| syn-grid-1000 | 1000 | 365.820 | 0.872 | 419x [404, 432] | 419x [402, 431] | 24.5 | 1 |
| syn-grid-2000 | 2000 | 2706.438 | 2.211 | 1,224x [1,202, 1,255] | 1,220x [1,198, 1,254] | 44.5 | 1 |
| syn-grid-5000 | 5000 | 19076.627 | 5.733 | 3,328x [3,271, 3,367] | 3,321x [3,265, 3,362] | 116.0 | 1 |
| syn-mix-0050 | 50 | 1.602 | 0.151 | 10.6x [9.36, 11.2] | 10.6x [9.29, 11.2] | 2.7 | 2 |
| syn-mix-0200 | 200 | 28.179 | 0.305 | 92.4x [87.4, 99.4] | 91.9x [87.3, 98.5] | 4.8 | 1 |
| syn-mix-0500 | 500 | 364.872 | 0.769 | 475x [466, 503] | 478x [466, 500] | 9.2 | 1 |
| syn-mix-1000 | 1000 | 2782.258 | 1.654 | 1,682x [1,665, 1,709] | 1,679x [1,660, 1,712] | 15.2 | 1 |
| syn-mix-2000 | 2000 | 21499.163 | 3.746 | 5,740x [5,615, 5,808] | 5,720x [5,611, 5,823] | 30.2 | 1 |
| syn-mix-5000 | 5000 | 23873.830 | 6.642 | 3,595x [3,523, 3,656] | 3,593x [3,535, 3,653] | 70.5 | 1 |

### real sketches: 105 timed

- @exact: 53 inputs; faster 23, slower 23, inconclusive 7; geometric-mean speedup 1.75x
- @jitter: 52 inputs; faster 38, slower 8, inconclusive 6; geometric-mean speedup 2.30x

| real inputs by constraint count | @exact: n, geo-mean speedup, min..max | @jitter: n, geo-mean speedup, min..max |
|---|---|---|
| 0-49 | 24, 1.06x, 0.88..4.44x | 24, 1.42x, 0.87..5.94x |
| 50-199 | 16, 1.80x, 0.84..7.97x | 16, 2.48x, 0.85..7.65x |
| 200-499 | 7, 3.00x, 0.93..6.12x | 7, 4.94x, 2.82..12.27x |
| 500-1999 | 4, 6.16x, 5.13..8.75x | 4, 5.66x, 4.72..8.96x |
| 2000-up | 2, 7.35x, 5.41..9.98x | 1, 9.76x, 9.76..9.76x |

| real input | constraints | old ms | new ms | wall speedup [CI] | verdict |
|---|---:|---:|---:|---|---|
| integration_tests_data_recover01.dxf@exact | 2634 | 102.188 | 18.880 | 5.41x [5.38, 5.47] | faster |
| examples_edgeminer_4_world.dxf@exact | 2437 | 205.573 | 20.608 | 9.98x [9.88, 10.1] | faster |
| examples_edgeminer_4_world.dxf@jitter | 2437 | 205.523 | 21.060 | 9.76x [8.92, 9.81] | faster |
| examples_edgeminer_3_us_main.dxf@exact | 1587 | 21.963 | 2.510 | 8.75x [8.56, 9.01] | faster |
| examples_edgeminer_3_us_main.dxf@jitter | 1587 | 21.925 | 2.447 | 8.96x [8.70, 9.06] | faster |
| examples_dxf_text_fonts.dxf@exact | 1512 | 9.642 | 1.878 | 5.13x [5.00, 5.30] | faster |
| examples_dxf_text_fonts.dxf@jitter | 1512 | 11.279 | 2.392 | 4.72x [4.63, 4.79] | faster |
| examples_dxf_text_mirror_shx_font.dxf@exact | 756 | 4.680 | 0.833 | 5.62x [5.37, 5.92] | faster |
| examples_dxf_text_mirror_shx_font.dxf@jitter | 756 | 5.196 | 1.057 | 4.91x [4.88, 5.08] | faster |
| examples_dxf_text_mirror_true_type_font.dxf@exact | 756 | 4.672 | 0.820 | 5.70x [5.60, 5.86] | faster |
| examples_dxf_text_mirror_true_type_font.dxf@jitter | 756 | 5.189 | 1.048 | 4.95x [4.82, 5.04] | faster |
| examples_dxf_text_stacked_shx_font.dxf@exact | 480 | 4.182 | 0.685 | 6.11x [5.92, 6.31] | faster |
| examples_dxf_text_stacked_shx_font.dxf@jitter | 480 | 5.261 | 0.882 | 5.97x [5.84, 6.13] | faster |
| examples_dxf_text_stacked_true_type_font.dxf@exact | 480 | 4.163 | 0.680 | 6.12x [5.92, 6.23] | faster |
| examples_dxf_text_stacked_true_type_font.dxf@jitter | 480 | 4.714 | 0.824 | 5.72x [5.58, 5.84] | faster |
| examples_dxf_text.dxf@exact | 383 | 0.726 | 0.200 | 3.64x [3.39, 3.85] | faster |
| examples_dxf_text.dxf@jitter | 383 | 1.314 | 0.464 | 2.83x [2.75, 2.94] | faster |
| examples_dxf_text_negative_extrusion.dxf@exact | 383 | 0.725 | 0.205 | 3.54x [3.00, 3.91] | faster |
| examples_dxf_text_negative_extrusion.dxf@jitter | 383 | 1.456 | 0.517 | 2.82x [2.74, 2.97] | faster |
| examples_dxf_text_oblique_rotate.dxf@exact | 344 | 1.796 | 0.358 | 5.02x [4.83, 5.25] | faster |
| examples_dxf_text_oblique_rotate.dxf@jitter | 344 | 2.420 | 0.510 | 4.74x [4.61, 5.00] | faster |
| examples_dxf_wipeout_door.dxf@exact | 332 | 0.130 | 0.135 | 0.97x [0.87, 1.08] | inconclusive |
| examples_dxf_wipeout_door.dxf@jitter | 332 | 8.908 | 0.726 | 12.3x [11.6, 12.5] | faster |
| integration_tests_data_duplicate_handles.dxf@exact | 220 | 0.140 | 0.151 | 0.93x [0.84, 1.05] | inconclusive |
| integration_tests_data_duplicate_handles.dxf@jitter | 220 | 1.961 | 0.434 | 4.51x [4.44, 4.78] | faster |
| examples_addons_optimize_forms.dxf@exact | 180 | 0.121 | 0.134 | 0.90x [0.80, 1.02] | inconclusive |
| examples_addons_optimize_forms.dxf@jitter | 180 | 0.205 | 0.234 | 0.88x [0.81, 1.04] | inconclusive |
| examples_dxf_image_images.dxf@exact | 164 | 0.106 | 0.114 | 0.93x [0.82, 0.99] | slower |
| examples_dxf_image_images.dxf@jitter | 164 | 0.153 | 0.111 | 1.38x [1.29, 1.48] | faster |
| examples_edgeminer_2_polylines.dxf@exact | 147 | 0.135 | 0.143 | 0.95x [0.83, 1.07] | inconclusive |
| examples_edgeminer_2_polylines.dxf@jitter | 147 | 1.351 | 0.263 | 5.14x [4.80, 5.48] | faster |
| examples_edgeminer_1_polylines.dxf@exact | 135 | 0.098 | 0.102 | 0.97x [0.82, 1.10] | inconclusive |
| examples_edgeminer_1_polylines.dxf@jitter | 135 | 1.044 | 0.288 | 3.63x [3.40, 3.84] | faster |
| examples_addons_drawing_data_usa.dxf@exact | 122 | 52.840 | 6.632 | 7.97x [7.88, 8.02] | faster |
| examples_addons_drawing_data_usa.dxf@jitter | 122 | 73.432 | 9.593 | 7.65x [7.57, 7.82] | faster |
| examples_dxf_columns_R2007.dxf@exact | 120 | 0.120 | 0.142 | 0.84x [0.81, 0.90] | slower |
| examples_dxf_columns_R2007.dxf@jitter | 120 | 0.150 | 0.177 | 0.85x [0.81, 0.89] | slower |
| examples_dxf_columns_R2018.dxf@exact | 120 | 0.123 | 0.141 | 0.87x [0.82, 0.94] | slower |
| examples_dxf_columns_R2018.dxf@jitter | 120 | 0.150 | 0.176 | 0.85x [0.81, 0.90] | slower |
| cadcraft-apartment.dxf@exact | 99 | 0.102 | 0.113 | 0.90x [0.82, 0.98] | slower |
| cadcraft-apartment.dxf@jitter | 99 | 0.157 | 0.159 | 0.99x [0.90, 1.07] | inconclusive |
| examples_dxf_hatches_1.dxf@exact | 84 | 0.122 | 0.107 | 1.14x [1.01, 1.28] | faster |
| examples_dxf_hatches_1.dxf@jitter | 84 | 0.206 | 0.129 | 1.59x [1.48, 1.69] | faster |
| integration_tests_data_POLI-ALL210_12.DXF@exact | 78 | 2.641 | 0.542 | 4.87x [4.78, 5.13] | faster |
| integration_tests_data_POLI-ALL210_12.DXF@jitter | 78 | 2.653 | 0.543 | 4.88x [4.63, 5.09] | faster |
| integration_tests_data_recover02.dxf@exact | 74 | 0.104 | 0.111 | 0.93x [0.86, 1.02] | inconclusive |
| integration_tests_data_recover02.dxf@jitter | 74 | 0.626 | 0.205 | 3.05x [3.01, 3.17] | faster |
| examples_dxf_uncommon.dxf@exact | 70 | 0.675 | 0.144 | 4.68x [4.25, 5.15] | faster |
| examples_dxf_uncommon.dxf@jitter | 70 | 0.685 | 0.126 | 5.45x [4.92, 5.98] | faster |
| examples_dxf_text_alignments.dxf@exact | 64 | 0.524 | 0.108 | 4.83x [4.53, 5.16] | faster |
| examples_dxf_text_alignments.dxf@jitter | 64 | 0.615 | 0.143 | 4.29x [4.02, 4.47] | faster |
| integration_tests_data_AC1003_LINE_Example.dxf@exact | 56 | 4.347 | 0.576 | 7.54x [7.02, 7.97] | faster |
| integration_tests_data_AC1003_LINE_Example.dxf@jitter | 56 | 4.384 | 0.583 | 7.51x [7.41, 8.19] | faster |
| examples_dxf_hatches_2.dxf@exact | 53 | 0.117 | 0.128 | 0.91x [0.85, 0.97] | slower |
| examples_dxf_hatches_2.dxf@jitter | 53 | 3.768 | 3.665 | 1.03x [1.02, 1.03] | faster |
| integration_tests_data_small_r13.dxf@exact | 52 | 0.356 | 0.094 | 3.80x [3.45, 4.17] | faster |
| integration_tests_data_small_r13.dxf@jitter | 52 | 0.374 | 0.095 | 3.94x [3.46, 4.14] | faster |
| examples_dxf_valid_lineweights.dxf@exact | 49 | 0.115 | 0.122 | 0.95x [0.91, 1.00] | slower |
| examples_dxf_valid_lineweights.dxf@jitter | 49 | 1.126 | 0.190 | 5.94x [5.70, 6.36] | faster |
| examples_dxf_insert_bricscad_level_1.dxf@exact | 34 | 0.111 | 0.118 | 0.94x [0.91, 1.00] | slower |
| examples_dxf_insert_bricscad_level_1.dxf@jitter | 34 | 0.305 | 0.137 | 2.22x [2.13, 2.29] | faster |
| examples_dxf_transform_hatch_pattern.dxf@exact | 33 | 0.126 | 0.136 | 0.92x [0.90, 0.96] | slower |
| examples_dxf_transform_hatch_pattern.dxf@jitter | 33 | 0.182 | 0.112 | 1.63x [1.54, 1.73] | faster |
| integration_tests_data_Leica_Disto_S910.dxf@exact | 31 | 0.460 | 0.104 | 4.44x [4.31, 4.93] | faster |
| integration_tests_data_Leica_Disto_S910.dxf@jitter | 31 | 0.916 | 0.199 | 4.60x [4.21, 4.98] | faster |
| integration_tests_data_mtext_framed_columns.dxf@exact | 31 | 0.134 | 0.140 | 0.95x [0.91, 0.98] | slower |
| integration_tests_data_mtext_framed_columns.dxf@jitter | 31 | 0.260 | 0.117 | 2.23x [2.14, 2.30] | faster |
| examples_dxf_3dface.dxf@exact | 24 | 0.128 | 0.138 | 0.93x [0.90, 0.98] | slower |
| examples_dxf_3dface.dxf@jitter | 24 | 0.162 | 0.100 | 1.62x [1.52, 1.68] | faster |
| examples_dxf_colors.dxf@exact | 23 | 0.124 | 0.134 | 0.93x [0.86, 0.98] | slower |
| examples_dxf_colors.dxf@jitter | 23 | 0.133 | 0.118 | 1.13x [0.98, 1.24] | inconclusive |
| integration_tests_data_empty_handles.dxf@exact | 16 | 0.148 | 0.156 | 0.95x [0.93, 0.96] | slower |
| integration_tests_data_empty_handles.dxf@jitter | 16 | 0.170 | 0.124 | 1.37x [1.31, 1.41] | faster |
| examples_dxf_image_viewports.dxf@exact | 12 | 0.126 | 0.136 | 0.93x [0.90, 0.98] | slower |
| examples_dxf_image_viewports.dxf@jitter | 12 | 0.148 | 0.099 | 1.50x [1.41, 1.58] | faster |
| examples_edgeminer_6_closed_loop_with_arcs.dxf@exact | 11 | 0.121 | 0.128 | 0.95x [0.92, 0.99] | slower |
| examples_edgeminer_6_closed_loop_with_arcs.dxf@jitter | 11 | 0.286 | 0.116 | 2.47x [2.38, 2.58] | faster |
| examples_dxf_hatch_issues.dxf@exact | 10 | 0.154 | 0.160 | 0.96x [0.93, 0.98] | slower |
| examples_dxf_hatch_issues.dxf@jitter | 10 | 0.191 | 0.111 | 1.72x [1.62, 1.79] | faster |
| tests_test_01_dxf_entities_houses_of_parliament_georeferenced.dxf@exact | 10 | 0.116 | 0.132 | 0.88x [0.83, 0.92] | slower |
| tests_test_01_dxf_entities_houses_of_parliament_georeferenced.dxf@jitter | 10 | 0.116 | 0.134 | 0.87x [0.83, 0.92] | slower |
| examples_dxf_hatch_text_box_in_fill.dxf@exact | 8 | 0.235 | 0.249 | 0.94x [0.87, 0.97] | slower |
| examples_dxf_hatch_text_box_in_fill.dxf@jitter | 8 | 0.150 | 0.170 | 0.89x [0.86, 0.92] | slower |
| examples_dxf_hatch_text_box_in_hole.dxf@exact | 8 | 0.245 | 0.262 | 0.94x [0.90, 0.95] | slower |
| examples_dxf_hatch_text_box_in_hole.dxf@jitter | 8 | 0.151 | 0.164 | 0.92x [0.60, 1.01] | inconclusive |
| examples_dxf_hatch_text_box_overlapping_fill_and_hole.dxf@exact | 8 | 0.237 | 0.252 | 0.94x [0.92, 0.96] | slower |
| examples_dxf_hatch_text_box_overlapping_fill_and_hole.dxf@jitter | 8 | 0.165 | 0.187 | 0.88x [0.86, 0.91] | slower |
| integration_tests_data_issue1106.dxf@exact | 8 | 0.209 | 0.118 | 1.76x [1.70, 1.88] | faster |
| integration_tests_data_issue1106.dxf@jitter | 8 | 0.155 | 0.083 | 1.87x [1.78, 2.08] | faster |
| integration_tests_data_custom_blocks.dxf@exact | 5 | 0.217 | 0.230 | 0.95x [0.91, 0.98] | slower |
| integration_tests_data_custom_blocks.dxf@jitter | 5 | 0.114 | 0.131 | 0.87x [0.85, 0.90] | slower |
| integration_tests_data_groups.dxf@exact | 5 | 0.165 | 0.120 | 1.37x [1.25, 1.48] | faster |
| integration_tests_data_groups.dxf@jitter | 5 | 0.184 | 0.131 | 1.41x [1.23, 1.53] | faster |
| examples_dxf_mtext_complex_inline_codes.dxf@exact | 4 | 0.219 | 0.232 | 0.94x [0.89, 0.98] | slower |
| examples_dxf_mtext_complex_inline_codes.dxf@jitter | 4 | 0.126 | 0.132 | 0.95x [0.87, 1.05] | inconclusive |
| integration_tests_data_mtext_text_frame.dxf@exact | 4 | 0.261 | 0.270 | 0.97x [0.93, 1.00] | slower |
| integration_tests_data_mtext_text_frame.dxf@jitter | 4 | 0.181 | 0.196 | 0.93x [0.89, 0.95] | slower |
| examples_dxf_VP4.dxf@exact | 3 | 0.230 | 0.241 | 0.96x [0.94, 0.98] | slower |
| examples_dxf_VP4.dxf@jitter | 3 | 0.142 | 0.153 | 0.92x [0.88, 0.95] | slower |
| examples_dxf_wipeout_random.dxf@exact | 3 | 0.124 | 0.126 | 0.98x [0.96, 1.02] | inconclusive |
| examples_dxf_wipeout_random.dxf@jitter | 3 | 0.134 | 0.134 | 1.00x [0.97, 1.04] | inconclusive |
| integration_tests_data_bin_dxf_r13.dxf@exact | 3 | 0.133 | 0.124 | 1.08x [1.05, 1.13] | faster |
| integration_tests_data_bin_dxf_r13.dxf@jitter | 3 | 0.154 | 0.142 | 1.09x [1.07, 1.11] | faster |
| integration_tests_data_bin_dxf_r14.dxf@exact | 3 | 0.190 | 0.177 | 1.08x [1.05, 1.11] | faster |
| integration_tests_data_bin_dxf_r14.dxf@jitter | 3 | 0.187 | 0.172 | 1.08x [1.06, 1.10] | faster |

### agreement

verdicts: {'agree': 129, 'both-fail-same': 2, 'DIFFER': 1}

max parameter difference over 130 comparable inputs: median 2.96e-11, p90 2.56e-09, max 1.39e-05

residual after solve (max over constraints): old max 2.42e-05, new max 2.42e-05

DOF/rank text equal pre-solve: 132/132; post-solve: 132/132

constraints skipped (uninterpretable): old 5020, new 5020

non-agreeing inputs:

- examples_edgeminer_5_us_states.dxf@exact (2285 cons): both-fail-same; old 'err: constraints conflict', new 'err: constraints conflict'; max diff None; post-residual old 4.97e-02 new 4.97e-02; dof post old `dof=Some(600) status=Under redundant=0 constraints=2285` new `dof=Some(600) status=Under redundant=0 constraints=2285`
- examples_edgeminer_5_us_states.dxf@jitter (2285 cons): both-fail-same; old 'err: constraints conflict', new 'err: constraints conflict'; max diff None; post-residual old 5.04e-02 new 5.04e-02; dof post old `dof=Some(600) status=Under redundant=0 constraints=2285` new `dof=Some(600) status=Under redundant=0 constraints=2285`
- integration_tests_data_recover01.dxf@jitter (2634 cons): DIFFER; old 'ok', new 'ok'; max diff 1.3908927940065041e-05; post-residual old 9.36e-07 new 9.36e-07; dof post old `dof=Some(824) status=Under redundant=0 constraints=2634` new `dof=Some(824) status=Under redundant=0 constraints=2634`

## arm main

Files: solver-main-real.json: publishable=True contended=False load 3.62->4.79 runs/side=10; solver-main-synthetic-fast.json: publishable=True contended=False load 4.79->4.64 runs/side=10

### synthetic speedup (old median / new median, wall; bootstrap 95% CI)

| constraints | chain | grid | mix | clustered |
|---:|---|---|---|---|
| 50 | 4.92x [4.40, 5.43] | 6.05x [5.59, 6.18] | 10.9x [10.0, 11.1] | 2.60x [2.45, 2.68] |
| 200 | 30.6x [29.9, 31.9] | 32.9x [32.3, 33.8] | 94.3x [90.8, 98.7] | 2.61x [2.50, 2.65] |
| 500 | 130x [124, 135] | 138x [127, 143] | not timed (one-sided-failure: old err: constraint system too large to solve) | 2.62x [2.60, 2.67] |
| 1000 | not timed (one-sided-failure: old err: constraint system too large to solve) | not timed (one-sided-failure: old err: constraint system too large to solve) | not timed (one-sided-failure: old err: constraint system too large to solve) | 2.55x [2.54, 2.60] |
| 2000 | not timed (one-sided-failure: old err: constraint system too large to solve) | not timed (one-sided-failure: old err: constraint system too large to solve) | not timed (one-sided-failure: old err: constraint system too large to solve) | 2.48x [2.45, 2.50] |
| 5000 | not timed (one-sided-failure: old err: constraint system too large to solve) | not timed (one-sided-failure: old err: constraint system too large to solve) | not timed (one-sided-failure: old err: constraint system too large to solve) | 2.48x [2.46, 2.52] |

### synthetic absolute medians (ms) old / new, and CPU-time speedup

| input | constraints | old ms | new ms | wall speedup | CPU speedup | clone us | batch |
|---|---:|---:|---:|---|---|---:|---:|
| syn-chain-0050 | 50 | 0.362 | 0.074 | 4.92x [4.40, 5.43] | 4.89x [4.39, 5.36] | 3.2 | 2 |
| syn-chain-0200 | 200 | 4.258 | 0.139 | 30.6x [29.9, 31.9] | 30.5x [29.9, 31.8] | 6.7 | 1 |
| syn-chain-0500 | 500 | 48.846 | 0.376 | 130x [124, 135] | 129x [124, 135] | 13.5 | 1 |
| syn-clustered-0050 | 50 | 0.328 | 0.126 | 2.60x [2.45, 2.68] | 2.60x [2.45, 2.70] | 2.8 | 1 |
| syn-clustered-0200 | 200 | 1.314 | 0.502 | 2.61x [2.50, 2.65] | 2.61x [2.56, 2.65] | 5.0 | 1 |
| syn-clustered-0500 | 486 | 3.401 | 1.297 | 2.62x [2.60, 2.67] | 2.63x [2.60, 2.66] | 8.7 | 1 |
| syn-clustered-1000 | 972 | 6.715 | 2.633 | 2.55x [2.54, 2.60] | 2.55x [2.53, 2.59] | 16.2 | 1 |
| syn-clustered-2000 | 1930 | 13.560 | 5.471 | 2.48x [2.45, 2.50] | 2.48x [2.45, 2.49] | 30.5 | 1 |
| syn-clustered-5000 | 4833 | 33.903 | 13.658 | 2.48x [2.46, 2.52] | 2.48x [2.47, 2.51] | 71.5 | 1 |
| syn-grid-0050 | 50 | 0.672 | 0.111 | 6.05x [5.59, 6.18] | 6.05x [5.50, 6.19] | 3.2 | 3 |
| syn-grid-0200 | 200 | 4.859 | 0.148 | 32.9x [32.3, 33.8] | 32.8x [32.1, 33.8] | 6.7 | 1 |
| syn-grid-0500 | 500 | 53.548 | 0.388 | 138x [127, 143] | 138x [127, 142] | 12.9 | 1 |
| syn-mix-0050 | 50 | 1.577 | 0.144 | 10.9x [10.0, 11.1] | 10.9x [10.0, 11.1] | 2.8 | 2 |
| syn-mix-0200 | 200 | 28.145 | 0.299 | 94.3x [90.8, 98.7] | 94.0x [91.0, 98.5] | 5.0 | 1 |

### real sketches: 105 timed

- @exact: 53 inputs; faster 23, slower 15, inconclusive 15; geometric-mean speedup 1.75x
- @jitter: 52 inputs; faster 40, slower 10, inconclusive 2; geometric-mean speedup 2.29x

| real inputs by constraint count | @exact: n, geo-mean speedup, min..max | @jitter: n, geo-mean speedup, min..max |
|---|---|---|
| 0-49 | 24, 1.07x, 0.88..4.60x | 24, 1.43x, 0.86..5.95x |
| 50-199 | 16, 1.78x, 0.84..8.01x | 16, 2.43x, 0.82..7.63x |
| 200-499 | 7, 2.96x, 0.93..5.98x | 7, 4.84x, 2.73..11.92x |
| 500-1999 | 4, 6.06x, 4.94..8.56x | 4, 5.55x, 4.60..8.62x |
| 2000-up | 2, 7.25x, 5.33..9.88x | 1, 9.51x, 9.51..9.51x |

| real input | constraints | old ms | new ms | wall speedup [CI] | verdict |
|---|---:|---:|---:|---|---|
| integration_tests_data_recover01.dxf@exact | 2634 | 101.403 | 19.041 | 5.33x [5.28, 5.40] | faster |
| examples_edgeminer_4_world.dxf@exact | 2437 | 206.575 | 20.910 | 9.88x [9.52, 10.1] | faster |
| examples_edgeminer_4_world.dxf@jitter | 2437 | 203.080 | 21.349 | 9.51x [9.40, 9.65] | faster |
| examples_edgeminer_3_us_main.dxf@exact | 1587 | 21.761 | 2.541 | 8.56x [8.38, 8.70] | faster |
| examples_edgeminer_3_us_main.dxf@jitter | 1587 | 22.122 | 2.566 | 8.62x [8.28, 8.95] | faster |
| examples_dxf_text_fonts.dxf@exact | 1512 | 9.832 | 1.989 | 4.94x [4.77, 5.27] | faster |
| examples_dxf_text_fonts.dxf@jitter | 1512 | 11.384 | 2.474 | 4.60x [4.50, 4.67] | faster |
| examples_dxf_text_mirror_shx_font.dxf@exact | 756 | 4.667 | 0.834 | 5.59x [5.26, 5.75] | faster |
| examples_dxf_text_mirror_shx_font.dxf@jitter | 756 | 5.252 | 1.077 | 4.88x [4.67, 5.10] | faster |
| examples_dxf_text_mirror_true_type_font.dxf@exact | 756 | 4.665 | 0.818 | 5.70x [5.40, 5.84] | faster |
| examples_dxf_text_mirror_true_type_font.dxf@jitter | 756 | 5.197 | 1.059 | 4.91x [4.68, 5.04] | faster |
| examples_dxf_text_stacked_shx_font.dxf@exact | 480 | 4.122 | 0.689 | 5.98x [5.75, 6.12] | faster |
| examples_dxf_text_stacked_shx_font.dxf@jitter | 480 | 5.306 | 0.969 | 5.47x [4.97, 6.07] | faster |
| examples_dxf_text_stacked_true_type_font.dxf@exact | 480 | 4.159 | 0.703 | 5.91x [5.55, 6.07] | faster |
| examples_dxf_text_stacked_true_type_font.dxf@jitter | 480 | 4.717 | 0.824 | 5.73x [5.58, 5.84] | faster |
| examples_dxf_text.dxf@exact | 383 | 0.731 | 0.210 | 3.48x [3.35, 3.86] | faster |
| examples_dxf_text.dxf@jitter | 383 | 1.305 | 0.479 | 2.73x [2.37, 2.96] | faster |
| examples_dxf_text_negative_extrusion.dxf@exact | 383 | 0.740 | 0.213 | 3.47x [3.23, 3.86] | faster |
| examples_dxf_text_negative_extrusion.dxf@jitter | 383 | 1.462 | 0.517 | 2.83x [2.70, 2.98] | faster |
| examples_dxf_text_oblique_rotate.dxf@exact | 344 | 1.774 | 0.348 | 5.09x [4.93, 5.21] | faster |
| examples_dxf_text_oblique_rotate.dxf@jitter | 344 | 2.426 | 0.516 | 4.70x [4.55, 4.86] | faster |
| examples_dxf_wipeout_door.dxf@exact | 332 | 0.133 | 0.135 | 0.98x [0.86, 1.09] | inconclusive |
| examples_dxf_wipeout_door.dxf@jitter | 332 | 8.810 | 0.739 | 11.9x [11.3, 12.3] | faster |
| integration_tests_data_duplicate_handles.dxf@exact | 220 | 0.136 | 0.147 | 0.93x [0.84, 1.06] | inconclusive |
| integration_tests_data_duplicate_handles.dxf@jitter | 220 | 1.964 | 0.427 | 4.60x [4.43, 4.79] | faster |
| examples_addons_optimize_forms.dxf@exact | 180 | 0.128 | 0.134 | 0.95x [0.85, 1.04] | inconclusive |
| examples_addons_optimize_forms.dxf@jitter | 180 | 0.204 | 0.236 | 0.87x [0.79, 0.92] | slower |
| examples_dxf_image_images.dxf@exact | 164 | 0.139 | 0.154 | 0.90x [0.62, 1.03] | inconclusive |
| examples_dxf_image_images.dxf@jitter | 164 | 0.160 | 0.113 | 1.41x [1.30, 1.59] | faster |
| examples_edgeminer_2_polylines.dxf@exact | 147 | 0.133 | 0.146 | 0.92x [0.83, 1.06] | inconclusive |
| examples_edgeminer_2_polylines.dxf@jitter | 147 | 1.349 | 0.276 | 4.88x [4.58, 5.37] | faster |
| examples_edgeminer_1_polylines.dxf@exact | 135 | 0.130 | 0.137 | 0.95x [0.84, 1.05] | inconclusive |
| examples_edgeminer_1_polylines.dxf@jitter | 135 | 1.051 | 0.295 | 3.57x [3.33, 3.79] | faster |
| examples_addons_drawing_data_usa.dxf@exact | 122 | 53.625 | 6.692 | 8.01x [7.77, 8.15] | faster |
| examples_addons_drawing_data_usa.dxf@jitter | 122 | 73.035 | 9.571 | 7.63x [7.59, 7.72] | faster |
| examples_dxf_columns_R2007.dxf@exact | 120 | 0.122 | 0.145 | 0.84x [0.79, 0.89] | slower |
| examples_dxf_columns_R2007.dxf@jitter | 120 | 0.149 | 0.182 | 0.82x [0.79, 0.85] | slower |
| examples_dxf_columns_R2018.dxf@exact | 120 | 0.123 | 0.145 | 0.85x [0.80, 0.91] | slower |
| examples_dxf_columns_R2018.dxf@jitter | 120 | 0.153 | 0.181 | 0.85x [0.77, 0.94] | slower |
| cadcraft-apartment.dxf@exact | 99 | 0.121 | 0.134 | 0.90x [0.84, 0.98] | slower |
| cadcraft-apartment.dxf@jitter | 99 | 0.157 | 0.162 | 0.97x [0.86, 1.03] | inconclusive |
| examples_dxf_hatches_1.dxf@exact | 84 | 0.122 | 0.107 | 1.14x [1.03, 1.27] | faster |
| examples_dxf_hatches_1.dxf@jitter | 84 | 0.223 | 0.138 | 1.62x [1.36, 1.84] | faster |
| integration_tests_data_POLI-ALL210_12.DXF@exact | 78 | 2.670 | 0.561 | 4.76x [4.58, 5.00] | faster |
| integration_tests_data_POLI-ALL210_12.DXF@jitter | 78 | 2.633 | 0.566 | 4.65x [4.14, 5.11] | faster |
| integration_tests_data_recover02.dxf@exact | 74 | 0.122 | 0.128 | 0.96x [0.88, 1.03] | inconclusive |
| integration_tests_data_recover02.dxf@jitter | 74 | 0.622 | 0.204 | 3.05x [2.95, 3.23] | faster |
| examples_dxf_uncommon.dxf@exact | 70 | 0.687 | 0.161 | 4.28x [4.11, 4.57] | faster |
| examples_dxf_uncommon.dxf@jitter | 70 | 0.688 | 0.127 | 5.41x [4.62, 5.97] | faster |
| examples_dxf_text_alignments.dxf@exact | 64 | 0.523 | 0.111 | 4.72x [4.51, 5.05] | faster |
| examples_dxf_text_alignments.dxf@jitter | 64 | 0.617 | 0.146 | 4.23x [4.04, 4.49] | faster |
| integration_tests_data_AC1003_LINE_Example.dxf@exact | 56 | 4.387 | 0.577 | 7.60x [7.30, 8.16] | faster |
| integration_tests_data_AC1003_LINE_Example.dxf@jitter | 56 | 4.416 | 0.592 | 7.46x [7.20, 7.86] | faster |
| examples_dxf_hatches_2.dxf@exact | 53 | 0.120 | 0.134 | 0.90x [0.79, 0.95] | slower |
| examples_dxf_hatches_2.dxf@jitter | 53 | 3.773 | 3.714 | 1.02x [1.00, 1.03] | faster |
| integration_tests_data_small_r13.dxf@exact | 52 | 0.528 | 0.139 | 3.80x [3.59, 4.07] | faster |
| integration_tests_data_small_r13.dxf@jitter | 52 | 0.375 | 0.110 | 3.41x [3.10, 4.01] | faster |
| examples_dxf_valid_lineweights.dxf@exact | 49 | 0.115 | 0.120 | 0.96x [0.91, 1.02] | inconclusive |
| examples_dxf_valid_lineweights.dxf@jitter | 49 | 1.130 | 0.190 | 5.95x [5.73, 6.44] | faster |
| examples_dxf_insert_bricscad_level_1.dxf@exact | 34 | 0.119 | 0.124 | 0.96x [0.92, 0.99] | slower |
| examples_dxf_insert_bricscad_level_1.dxf@jitter | 34 | 0.318 | 0.140 | 2.27x [2.08, 2.59] | faster |
| examples_dxf_transform_hatch_pattern.dxf@exact | 33 | 0.127 | 0.135 | 0.94x [0.90, 0.99] | slower |
| examples_dxf_transform_hatch_pattern.dxf@jitter | 33 | 0.188 | 0.118 | 1.59x [1.42, 1.72] | faster |
| integration_tests_data_Leica_Disto_S910.dxf@exact | 31 | 0.460 | 0.100 | 4.60x [4.21, 4.95] | faster |
| integration_tests_data_Leica_Disto_S910.dxf@jitter | 31 | 0.933 | 0.199 | 4.69x [4.48, 5.11] | faster |
| integration_tests_data_mtext_framed_columns.dxf@exact | 31 | 0.134 | 0.138 | 0.97x [0.92, 1.00] | slower |
| integration_tests_data_mtext_framed_columns.dxf@jitter | 31 | 0.258 | 0.117 | 2.21x [2.06, 2.35] | faster |
| examples_dxf_3dface.dxf@exact | 24 | 0.135 | 0.139 | 0.97x [0.93, 1.12] | inconclusive |
| examples_dxf_3dface.dxf@jitter | 24 | 0.194 | 0.120 | 1.62x [1.48, 1.70] | faster |
| examples_dxf_colors.dxf@exact | 23 | 0.125 | 0.135 | 0.93x [0.87, 1.05] | inconclusive |
| examples_dxf_colors.dxf@jitter | 23 | 0.135 | 0.114 | 1.19x [1.08, 1.27] | faster |
| integration_tests_data_empty_handles.dxf@exact | 16 | 0.145 | 0.154 | 0.94x [0.92, 0.97] | slower |
| integration_tests_data_empty_handles.dxf@jitter | 16 | 0.172 | 0.124 | 1.38x [1.33, 1.46] | faster |
| examples_dxf_image_viewports.dxf@exact | 12 | 0.142 | 0.154 | 0.92x [0.87, 1.04] | inconclusive |
| examples_dxf_image_viewports.dxf@jitter | 12 | 0.169 | 0.113 | 1.49x [1.38, 1.56] | faster |
| examples_edgeminer_6_closed_loop_with_arcs.dxf@exact | 11 | 0.123 | 0.129 | 0.95x [0.88, 1.00] | inconclusive |
| examples_edgeminer_6_closed_loop_with_arcs.dxf@jitter | 11 | 0.288 | 0.116 | 2.48x [2.35, 2.77] | faster |
| examples_dxf_hatch_issues.dxf@exact | 10 | 0.143 | 0.150 | 0.96x [0.94, 0.98] | slower |
| examples_dxf_hatch_issues.dxf@jitter | 10 | 0.211 | 0.125 | 1.69x [1.43, 1.77] | faster |
| tests_test_01_dxf_entities_houses_of_parliament_georeferenced.dxf@exact | 10 | 0.121 | 0.137 | 0.88x [0.76, 0.93] | slower |
| tests_test_01_dxf_entities_houses_of_parliament_georeferenced.dxf@jitter | 10 | 0.118 | 0.132 | 0.89x [0.84, 0.94] | slower |
| examples_dxf_hatch_text_box_in_fill.dxf@exact | 8 | 0.227 | 0.237 | 0.96x [0.94, 0.98] | slower |
| examples_dxf_hatch_text_box_in_fill.dxf@jitter | 8 | 0.157 | 0.179 | 0.88x [0.86, 0.90] | slower |
| examples_dxf_hatch_text_box_in_hole.dxf@exact | 8 | 0.159 | 0.169 | 0.94x [0.93, 1.00] | slower |
| examples_dxf_hatch_text_box_in_hole.dxf@jitter | 8 | 0.120 | 0.136 | 0.88x [0.84, 0.91] | slower |
| examples_dxf_hatch_text_box_overlapping_fill_and_hole.dxf@exact | 8 | 0.230 | 0.246 | 0.93x [0.82, 1.04] | inconclusive |
| examples_dxf_hatch_text_box_overlapping_fill_and_hole.dxf@jitter | 8 | 0.151 | 0.173 | 0.87x [0.85, 0.90] | slower |
| integration_tests_data_issue1106.dxf@exact | 8 | 0.210 | 0.119 | 1.75x [1.67, 1.89] | faster |
| integration_tests_data_issue1106.dxf@jitter | 8 | 0.222 | 0.110 | 2.02x [1.94, 2.14] | faster |
| integration_tests_data_custom_blocks.dxf@exact | 5 | 0.229 | 0.235 | 0.97x [0.96, 1.02] | inconclusive |
| integration_tests_data_custom_blocks.dxf@jitter | 5 | 0.139 | 0.161 | 0.86x [0.84, 0.91] | slower |
| integration_tests_data_groups.dxf@exact | 5 | 0.182 | 0.121 | 1.50x [1.44, 1.57] | faster |
| integration_tests_data_groups.dxf@jitter | 5 | 0.180 | 0.119 | 1.51x [1.38, 1.63] | faster |
| examples_dxf_mtext_complex_inline_codes.dxf@exact | 4 | 0.213 | 0.223 | 0.96x [0.93, 0.99] | slower |
| examples_dxf_mtext_complex_inline_codes.dxf@jitter | 4 | 0.120 | 0.129 | 0.93x [0.87, 1.03] | inconclusive |
| integration_tests_data_mtext_text_frame.dxf@exact | 4 | 0.283 | 0.290 | 0.98x [0.96, 0.99] | slower |
| integration_tests_data_mtext_text_frame.dxf@jitter | 4 | 0.177 | 0.195 | 0.91x [0.88, 0.96] | slower |
| examples_dxf_VP4.dxf@exact | 3 | 0.212 | 0.223 | 0.95x [0.92, 0.98] | slower |
| examples_dxf_VP4.dxf@jitter | 3 | 0.145 | 0.157 | 0.93x [0.90, 0.98] | slower |
| examples_dxf_wipeout_random.dxf@exact | 3 | 0.127 | 0.125 | 1.02x [0.95, 1.07] | inconclusive |
| examples_dxf_wipeout_random.dxf@jitter | 3 | 0.143 | 0.134 | 1.06x [1.02, 1.12] | faster |
| integration_tests_data_bin_dxf_r13.dxf@exact | 3 | 0.136 | 0.125 | 1.08x [1.06, 1.15] | faster |
| integration_tests_data_bin_dxf_r13.dxf@jitter | 3 | 0.159 | 0.150 | 1.06x [1.03, 1.10] | faster |
| integration_tests_data_bin_dxf_r14.dxf@exact | 3 | 0.185 | 0.169 | 1.10x [1.04, 1.12] | faster |
| integration_tests_data_bin_dxf_r14.dxf@jitter | 3 | 0.178 | 0.164 | 1.08x [1.06, 1.10] | faster |

### agreement

verdicts: {'agree': 119, 'one-sided-failure': 10, 'both-fail-same': 2, 'DIFFER': 1}

max parameter difference over 120 comparable inputs: median 2.60e-11, p90 3.62e-09, max 1.39e-05

residual after solve (max over constraints): old max 2.42e-05, new max 2.42e-05

DOF/rank text equal pre-solve: 132/132; post-solve: 132/132

constraints skipped (uninterpretable): old 0, new 5020

non-agreeing inputs:

- syn-mix-0500 (500 cons): one-sided-failure; old 'err: constraint system too large to solve', new 'ok'; max diff None; post-residual old 1.30e-01 new 2.72e-08; dof post old `dof=None status=Unknown redundant=0 constraints=500` new `dof=None status=Unknown redundant=0 constraints=500`
- syn-chain-1000 (1000 cons): one-sided-failure; old 'err: constraint system too large to solve', new 'ok'; max diff None; post-residual old 1.84e-02 new 5.31e-11; dof post old `dof=None status=Unknown redundant=0 constraints=1000` new `dof=None status=Unknown redundant=0 constraints=1000`
- syn-grid-1000 (1000 cons): one-sided-failure; old 'err: constraint system too large to solve', new 'ok'; max diff None; post-residual old 1.00e-01 new 3.92e-11; dof post old `dof=None status=Unknown redundant=0 constraints=1000` new `dof=None status=Unknown redundant=0 constraints=1000`
- syn-mix-1000 (1000 cons): one-sided-failure; old 'err: constraint system too large to solve', new 'ok'; max diff None; post-residual old 1.33e-01 new 7.09e-08; dof post old `dof=None status=Unknown redundant=0 constraints=1000` new `dof=None status=Unknown redundant=0 constraints=1000`
- syn-chain-2000 (2000 cons): one-sided-failure; old 'err: constraint system too large to solve', new 'ok'; max diff None; post-residual old 1.84e-02 new 5.39e-11; dof post old `dof=None status=Unknown redundant=0 constraints=2000` new `dof=None status=Unknown redundant=0 constraints=2000`
- syn-grid-2000 (2000 cons): one-sided-failure; old 'err: constraint system too large to solve', new 'ok'; max diff None; post-residual old 1.00e-01 new 9.13e-11; dof post old `dof=None status=Unknown redundant=0 constraints=2000` new `dof=None status=Unknown redundant=0 constraints=2000`
- syn-mix-2000 (2000 cons): one-sided-failure; old 'err: constraint system too large to solve', new 'ok'; max diff None; post-residual old 1.37e-01 new 3.42e-08; dof post old `dof=None status=Unknown redundant=0 constraints=2000` new `dof=None status=Unknown redundant=0 constraints=2000`
- syn-chain-5000 (5000 cons): one-sided-failure; old 'err: constraint system too large to solve', new 'ok'; max diff None; post-residual old 1.84e-02 new 5.38e-11; dof post old `dof=None status=Unknown redundant=0 constraints=3998` new `dof=None status=Unknown redundant=0 constraints=3998`
- syn-grid-5000 (5000 cons): one-sided-failure; old 'err: constraint system too large to solve', new 'ok'; max diff None; post-residual old 1.00e-01 new 1.20e-10; dof post old `dof=None status=Unknown redundant=0 constraints=3909` new `dof=None status=Unknown redundant=0 constraints=3909`
- syn-mix-5000 (5000 cons): one-sided-failure; old 'err: constraint system too large to solve', new 'ok'; max diff None; post-residual old 1.75e-01 new 1.19e-07; dof post old `dof=None status=Unknown redundant=0 constraints=2073` new `dof=None status=Unknown redundant=0 constraints=2073`
- examples_edgeminer_5_us_states.dxf@exact (2285 cons): both-fail-same; old 'err: constraints conflict', new 'err: constraints conflict'; max diff None; post-residual old 4.97e-02 new 4.97e-02; dof post old `dof=Some(600) status=Under redundant=0 constraints=2285` new `dof=Some(600) status=Under redundant=0 constraints=2285`
- examples_edgeminer_5_us_states.dxf@jitter (2285 cons): both-fail-same; old 'err: constraints conflict', new 'err: constraints conflict'; max diff None; post-residual old 5.04e-02 new 5.04e-02; dof post old `dof=Some(600) status=Under redundant=0 constraints=2285` new `dof=Some(600) status=Under redundant=0 constraints=2285`
- integration_tests_data_recover01.dxf@jitter (2634 cons): DIFFER; old 'ok', new 'ok'; max diff 1.3908927940065041e-05; post-residual old 9.36e-07 new 9.36e-07; dof post old `dof=Some(824) status=Under redundant=0 constraints=2634` new `dof=Some(824) status=Under redundant=0 constraints=2634`

