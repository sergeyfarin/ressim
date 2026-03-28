#!/bin/bash
cat << 'PATCH' > wells.patch
--- src/lib/ressim/src/fim/wells.rs
+++ src/lib/ressim/src/fim/wells.rs
@@ -1134,8 +1134,13 @@ pub(crate) fn well_constraint_residual(
         return Some(bhp_bar - control.bhp_target);
     }
 
     let (bhp_slack, rate_slack) = well_control_slacks(sim, state, topology, well_idx)?;
-    Some(fischer_burmeister(bhp_slack, rate_slack))
+    // Scale them to make them comparable and unitless
+    let target_rate = control.target_rate.unwrap_or(1.0).max(1.0);
+    let limit = control.bhp_limit.max(1.0);
+    
+    let scaled_bhp_slack = bhp_slack / limit;
+    let scaled_rate_slack = rate_slack / target_rate;
+    Some(fischer_burmeister(scaled_bhp_slack, scaled_rate_slack))
 }
 
 pub(crate) fn well_control_slacks(
PATCH
patch -p0 < wells.patch
