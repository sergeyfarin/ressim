#!/bin/bash
cat << 'PATCH' > assembly_fb.patch
--- src/lib/ressim/src/fim/assembly.rs
+++ src/lib/ressim/src/fim/assembly.rs
@@ -141,8 +141,12 @@ fn add_exact_well_constraint_jacobian(
         let Some((bhp_slack, rate_slack)) = well_control_slacks(sim, state, topology, well_idx) else {
             continue;
         };
-        let (dphi_da, dphi_db) = fischer_burmeister_gradient(bhp_slack, rate_slack);
+        let target_rate = control.target_rate.unwrap_or(1.0).max(1.0);
+        let limit = control.bhp_limit.max(1.0);
+        let scaled_bhp_slack = bhp_slack / limit;
+        let scaled_rate_slack = rate_slack / target_rate;
+        let (dphi_da_scaled, dphi_db_scaled) = fischer_burmeister_gradient(scaled_bhp_slack, scaled_rate_slack);
+        let dphi_da = dphi_da_scaled / limit;
+        let dphi_db = dphi_db_scaled / target_rate;
 
         let mut row_entries = Vec::new();
         if well.injector {
@@ -174,8 +178,12 @@ fn add_exact_well_constraint_cell_jacobian(
         let Some((bhp_slack, rate_slack)) = well_control_slacks(sim, state, topology, well_idx) else {
             continue;
         };
-        let (_, dphi_db) = fischer_burmeister_gradient(bhp_slack, rate_slack);
+        let target_rate = control.target_rate.unwrap_or(1.0).max(1.0);
+        let limit = control.bhp_limit.max(1.0);
+        let scaled_bhp_slack = bhp_slack / limit;
+        let scaled_rate_slack = rate_slack / target_rate;
+        let (_, dphi_db_scaled) = fischer_burmeister_gradient(scaled_bhp_slack, scaled_rate_slack);
+        let dphi_db = dphi_db_scaled / target_rate;
 
         for perf_idx in topology.well_perforations(well_idx) {
             let ref_perf = &topology.perforations[perf_idx];
PATCH
patch -p0 < assembly_fb.patch
