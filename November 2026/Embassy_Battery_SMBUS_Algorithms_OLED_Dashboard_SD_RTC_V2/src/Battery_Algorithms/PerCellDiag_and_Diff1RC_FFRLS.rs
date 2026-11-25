#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

// ============================================================================
// Constants & Configuration
// ============================================================================
pub const MAX_PACK_CELLS: usize = 15;

/// Minimum cell voltage (mV) to consider a channel populated (11S -> C1..C11)
const ACTIVE_CELL_MIN_MV: u16 = 500;

/// Minimum current step (A) required to trigger step-DCIR & FFRLS covariance updates
const STEP_CURRENT_MIN_A: f32 = 0.15;

/// Forgetting factor lambda in (0.95, 0.999]; 0.985 balances tracking & noise rejection
const FFRLS_FORGETTING_FACTOR: f32 = 0.985;

// ============================================================================
// Output Data Structures
// ============================================================================
#[derive(Debug, Clone, Copy)]
pub struct PerCellDiagOutput
{
    pub active_cells: u8,
    pub v_min_mv: u16,
    pub v_max_mv: u16,
    pub v_spread_mv: u16,
    pub v_mean_mv: f32,
    pub v_std_mv: f32,
    /// 1-based index of lowest and highest voltage cell (1..=15)
    pub min_cell_idx: u8,
    pub max_cell_idx: u8,
    /// Standardized Z-score per cell: (V_i - V_mean) / V_std
    pub cell_z_score: [f32; MAX_PACK_CELLS],
    /// EMA-filtered step ohmic resistance R0 per cell (mOhm)
    pub cell_r0_mohm: [f32; MAX_PACK_CELLS],
    pub r0_mean_mohm: f32,
    pub r0_max_mohm: f32,
    /// 1-based index of the highest-resistance (weakest) cell
    pub r0_max_cell_idx: u8,
    pub step_updated: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Diff1RcFfrlsOutput
{
    /// Ohmic series resistance R0 (mOhm)
    pub r0_mohm: f32,
    /// Electrochemical polarization resistance R1 (mOhm)
    pub r1_mohm: f32,
    /// Polarization capacitance C1 (Farads)
    pub c1_farad: f32,
    /// Polarization time constant tau = R1 * C1 (seconds)
    pub tau_s: f32,
    /// One-step-ahead terminal voltage prediction error (mV)
    pub v_pred_err_mv: f32,
    /// True if current excitation exceeded threshold and RLS updated this tick
    pub ffrls_updated: bool,
    /// Total valid excitation updates accumulated since boot
    pub update_count: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct PerCellDiag_and_Diff1RC_Output
{
    pub cell_diag: PerCellDiagOutput,
    pub ffrls: Diff1RcFfrlsOutput,
}

// ============================================================================
// Combined Algorithm State Engine
// ============================================================================
pub struct PerCellDiag_and_Diff1RC_FFRLS
{
    // --- Per-Cell Step Resistance State ---
    prev_cell_mv: Option<[u16; MAX_PACK_CELLS]>,
    cell_r0_mohm: [f32; MAX_PACK_CELLS],

    // --- Differential 1-RC FFRLS State ---
    prev_v_pack_v: Option<f32>,
    prev_i_pack_a: Option<f32>,
    prev_dv_v: f32,
    prev_di_a: f32,
    has_diff_history: bool,

    /// Parameter vector theta = [a1, b0, b1]
    theta: [f32; 3],
    /// 3x3 RLS error covariance matrix P
    p_mat: [[f32; 3]; 3],

    // --- Last Valid Physical ECM Parameters ---
    r0_mohm: f32,
    r1_mohm: f32,
    c1_farad: f32,
    tau_s: f32,
    update_count: u32,
}

impl PerCellDiag_and_Diff1RC_FFRLS
{
    /// Initializes the engine with physically realistic 11S-2P priors:
    /// - `r0_init_mohm`: e.g. 85.0 mOhm pack (~7.7 mOhm/cell)
    /// - `r1_init_mohm`: e.g. 30.0 mOhm polarization resistance
    /// - `c1_init_farad`: e.g. 1500.0 F polarization capacitance (tau = 45s)
    /// - `nominal_dt_s`: nominal sample period (e.g. 1.0 s)
    pub fn new
    (
        r0_init_mohm: f32,
        r1_init_mohm: f32,
        c1_init_farad: f32,
        nominal_dt_s: f32,
    ) -> Self
    {
        let r0_ohm = r0_init_mohm / 1000.0;
        let r1_ohm = r1_init_mohm / 1000.0;
        let c1_farad = c1_init_farad;
        let tau = (r1_ohm * c1_farad).max(1.0);
        let dt = nominal_dt_s.max(0.05);

        // Bilinear (Tustin) transform mapping from physical (R0, R1, tau) -> theta = [a1, b0, b1]
        let k = (2.0 * tau) / dt;
        let denom = k + 1.0;
        let a1 = (k - 1.0) / denom;
        let b0 = (r0_ohm * denom + r1_ohm) / denom;
        let b1 = (r0_ohm * (1.0 - k) + r1_ohm) / denom;

        let active_cells_guess = 11.0;
        let init_cell_r0 = r0_init_mohm / active_cells_guess;

        Self
        {
            prev_cell_mv: None,
            cell_r0_mohm: [init_cell_r0; MAX_PACK_CELLS],

            prev_v_pack_v: None,
            prev_i_pack_a: None,
            prev_dv_v: 0.0,
            prev_di_a: 0.0,
            has_diff_history: false,

            theta: [a1, b0, b1],
            p_mat:
            [
                [10.0, 0.0,  0.0],
                [0.0,  10.0, 0.0],
                [0.0,  0.0,  10.0],
            ],

            r0_mohm: r0_init_mohm,
            r1_mohm: r1_init_mohm,
            c1_farad,
            tau_s: tau,
            update_count: 0,
        }
    }

    /// Resets transient history (e.g. when battery is disconnected / PRES goes high)
    pub fn reset_transients(&mut self)
    {
        self.prev_cell_mv = None;
        self.prev_v_pack_v = None;
        self.prev_i_pack_a = None;
        self.prev_dv_v = 0.0;
        self.prev_di_a = 0.0;
        self.has_diff_history = false;
    }

    /// Executes one step of Per-Cell Diagnostics + Differential 1-RC FFRLS.
    ///
    /// - `pack_voltage_mv`: Pack terminal voltage in mV
    /// - `current_ma`: Pack current in mA (+Charge, -Discharge)
    /// - `cell_voltages_mv`: Slice of up to 15 cell voltages in mV
    /// - `dt_s`: Elapsed time since previous sample in seconds (e.g. 1.0 or 0.25)
    pub fn update
    (
        &mut self,
        pack_voltage_mv: u32,
        current_ma: i32,
        cell_voltages_mv: &[u16],
        dt_s: f32,
    ) -> PerCellDiag_and_Diff1RC_Output
    {
        let v_pack_v = (pack_voltage_mv as f32) / 1000.0;
        let i_pack_a = (current_ma as f32) / 1000.0;
        let dt = dt_s.clamp(0.05, 10.0);

        let di_a = match self.prev_i_pack_a
        {
            Some(ip) => i_pack_a - ip,
            None => 0.0,
        };

        // --------------------------------------------------------------------
        // 1. Per-Cell Consistency & Step Ohmic Resistance (C1..C15)
        // --------------------------------------------------------------------
        let cell_diag = self.step_per_cell_diagnostics(cell_voltages_mv, di_a);

        // --------------------------------------------------------------------
        // 2. Pack-Level Differential 1-RC FFRLS
        // --------------------------------------------------------------------
        let ffrls = self.step_diff_1rc_ffrls(v_pack_v, i_pack_a, dt);

        PerCellDiag_and_Diff1RC_Output { cell_diag, ffrls }
    }

    // ========================================================================
    // Internal: Per-Cell Diagnostics
    // ========================================================================
    fn step_per_cell_diagnostics
    (
        &mut self,
        cell_voltages_mv: &[u16],
        di_a: f32,
    ) -> PerCellDiagOutput
    {
        let n = cell_voltages_mv.len().min(MAX_PACK_CELLS);
        let mut cells_now = [0u16; MAX_PACK_CELLS];
        cells_now[..n].copy_from_slice(&cell_voltages_mv[..n]);

        let mut active_cells: u8 = 0;
        let mut v_min: u16 = u16::MAX;
        let mut v_max: u16 = 0;
        let mut min_idx: u8 = 0;
        let mut max_idx: u8 = 0;
        let mut sum_v: f32 = 0.0;

        for i in 0..n
        {
            let v = cells_now[i];
            if v >= ACTIVE_CELL_MIN_MV
            {
                active_cells += 1;
                sum_v += v as f32;
                if v < v_min
                {
                    v_min = v;
                    min_idx = (i + 1) as u8;
                }
                if v > v_max
                {
                    v_max = v;
                    max_idx = (i + 1) as u8;
                }
            }
        }

        if active_cells == 0
        {
            v_min = 0;
        }

        let v_mean = if active_cells > 0
        {
            sum_v / (active_cells as f32)
        }
        else
        {
            0.0
        };

        let mut var_sum: f32 = 0.0;
        for i in 0..n
        {
            let v = cells_now[i];
            if v >= ACTIVE_CELL_MIN_MV
            {
                let diff = (v as f32) - v_mean;
                var_sum += diff * diff;
            }
        }

        let v_std = if active_cells > 1
        {
            sqrt_f32(var_sum / (active_cells as f32))
        }
        else
        {
            0.0
        };

        let mut z_scores = [0.0f32; MAX_PACK_CELLS];
        if v_std > 0.05
        {
            for i in 0..n
            {
                if cells_now[i] >= ACTIVE_CELL_MIN_MV
                {
                    z_scores[i] = ((cells_now[i] as f32) - v_mean) / v_std;
                }
            }
        }

        // Per-cell instantaneous step ohmic resistance R0_i = dV_i / dI
        let mut step_updated = false;
        if let Some(prev_cells) = self.prev_cell_mv
        {
            if abs_f32(di_a) >= STEP_CURRENT_MIN_A
            {
                for i in 0..n
                {
                    if cells_now[i] >= ACTIVE_CELL_MIN_MV && prev_cells[i] >= ACTIVE_CELL_MIN_MV
                    {
                        let dv_mv = (cells_now[i] as f32) - (prev_cells[i] as f32);
                        let inst_r0_mohm = dv_mv / di_a; // mV / A = mOhm

                        // Physical bounds per cell: 1.0 mOhm to 150.0 mOhm
                        if inst_r0_mohm >= 1.0 && inst_r0_mohm <= 150.0
                        {
                            self.cell_r0_mohm[i] =
                                0.85 * self.cell_r0_mohm[i] + 0.15 * inst_r0_mohm;
                            step_updated = true;
                        }
                    }
                }
            }
        }
        self.prev_cell_mv = Some(cells_now);

        let mut r0_sum: f32 = 0.0;
        let mut r0_max: f32 = 0.0;
        let mut r0_max_idx: u8 = 0;

        for i in 0..n
        {
            if cells_now[i] >= ACTIVE_CELL_MIN_MV
            {
                let r = self.cell_r0_mohm[i];
                r0_sum += r;
                if r > r0_max
                {
                    r0_max = r;
                    r0_max_idx = (i + 1) as u8;
                }
            }
        }

        let r0_mean = if active_cells > 0
        {
            r0_sum / (active_cells as f32)
        }
        else
        {
            0.0
        };

        PerCellDiagOutput
        {
            active_cells,
            v_min_mv: v_min,
            v_max_mv: v_max,
            v_spread_mv: v_max.saturating_sub(v_min),
            v_mean_mv: v_mean,
            v_std_mv: v_std,
            min_cell_idx: min_idx,
            max_cell_idx: max_idx,
            cell_z_score: z_scores,
            cell_r0_mohm: self.cell_r0_mohm,
            r0_mean_mohm: r0_mean,
            r0_max_mohm: r0_max,
            r0_max_cell_idx: r0_max_idx,
            step_updated,
        }
    }

    // ========================================================================
    // Internal: Differential 1-RC FFRLS
    // ========================================================================
    fn step_diff_1rc_ffrls
    (
        &mut self,
        v_pack_v: f32,
        i_pack_a: f32,
        dt_s: f32,
    ) -> Diff1RcFfrlsOutput
    {
        let mut v_pred_err_mv = 0.0;
        let mut ffrls_updated = false;

        if let (Some(vp), Some(ip)) = (self.prev_v_pack_v, self.prev_i_pack_a)
        {
            let dv_v = v_pack_v - vp;
            let di_a = i_pack_a - ip;

            if self.has_diff_history
            {
                // Regressor phi = [dV(k-1), dI(k), dI(k-1)]
                let phi = [self.prev_dv_v, di_a, self.prev_di_a];

                // Prior voltage-change prediction: dV_hat = phi^T * theta
                let dv_hat =
                    phi[0] * self.theta[0] + phi[1] * self.theta[1] + phi[2] * self.theta[2];
                let err_v = dv_v - dv_hat;
                v_pred_err_mv = err_v * 1000.0;

                // Persistent excitation guard: freeze RLS when current is idle/flat
                if abs_f32(di_a) >= STEP_CURRENT_MIN_A || abs_f32(self.prev_di_a) >= STEP_CURRENT_MIN_A
                {
                    // P * phi
                    let mut p_phi = [0.0f32; 3];
                    for r in 0..3
                    {
                        p_phi[r] = self.p_mat[r][0] * phi[0]
                            + self.p_mat[r][1] * phi[1]
                            + self.p_mat[r][2] * phi[2];
                    }

                    // Denominator: lambda + phi^T * P * phi
                    let phi_p_phi = phi[0] * p_phi[0] + phi[1] * p_phi[1] + phi[2] * p_phi[2];
                    let denom = FFRLS_FORGETTING_FACTOR + phi_p_phi;

                    if denom > 1e-6
                    {
                        // Kalman/RLS gain K = (P * phi) / (lambda + phi^T * P * phi)
                        let k_gain = [p_phi[0] / denom, p_phi[1] / denom, p_phi[2] / denom];

                        // Parameter update: theta(k) = theta(k-1) + K * err
                        for idx in 0..3
                        {
                            self.theta[idx] += k_gain[idx] * err_v;
                        }

                        // Covariance update: P(k) = (P(k-1) - K * (P * phi)^T) / lambda
                        let inv_lambda = 1.0 / FFRLS_FORGETTING_FACTOR;
                        for r in 0..3
                        {
                            for c in 0..3
                            {
                                self.p_mat[r][c] =
                                    (self.p_mat[r][c] - k_gain[r] * p_phi[c]) * inv_lambda;
                            }
                        }

                        // Extract physical (R0, R1, C1) via inverse bilinear transform
                        let a1 = self.theta[0];
                        let b0 = self.theta[1];
                        let b1 = self.theta[2];

                        if a1 > 0.05 && a1 < 0.995
                        {
                            let r0_ohm = (b0 - b1) / (1.0 + a1);
                            let tau = 0.5 * dt_s * ((1.0 + a1) / (1.0 - a1));
                            let r1_ohm = ((b0 + b1) / (1.0 - a1)) - r0_ohm;

                            let r0_m = r0_ohm * 1000.0;
                            let r1_m = r1_ohm * 1000.0;

                            if r0_m >= 5.0 && r0_m <= 1000.0 && r1_m >= 1.0 && r1_m <= 1000.0
                            {
                                let c1_f = tau / r1_ohm;
                                if c1_f >= 10.0 && c1_f <= 50_000.0
                                {
                                    // Light smoothing on physical outputs to suppress SMBus quantization jitter
                                    self.r0_mohm = 0.80 * self.r0_mohm + 0.20 * r0_m;
                                    self.r1_mohm = 0.80 * self.r1_mohm + 0.20 * r1_m;
                                    self.c1_farad = 0.80 * self.c1_farad + 0.20 * c1_f;
                                    self.tau_s = (self.r1_mohm / 1000.0) * self.c1_farad;
                                    ffrls_updated = true;
                                    self.update_count += 1;
                                }
                            }
                        }
                    }
                }
            }

            self.prev_dv_v = dv_v;
            self.prev_di_a = di_a;
            self.has_diff_history = true;
        }

        self.prev_v_pack_v = Some(v_pack_v);
        self.prev_i_pack_a = Some(i_pack_a);

        Diff1RcFfrlsOutput
        {
            r0_mohm: self.r0_mohm,
            r1_mohm: self.r1_mohm,
            c1_farad: self.c1_farad,
            tau_s: self.tau_s,
            v_pred_err_mv,
            ffrls_updated,
            update_count: self.update_count,
        }
    }
}

// ============================================================================
// no_std Math Helpers (Zero External Crate Dependencies)
// ============================================================================
#[inline]
const fn abs_f32(x: f32) -> f32
{
    if x < 0.0 { -x } else { x }
}

#[inline]
fn sqrt_f32(x: f32) -> f32
{
    if x <= 0.0
    {
        return 0.0;
    }
    let mut guess = x * 0.5;
    if guess < 1.0
    {
        guess = 1.0;
    }
    // 6 Newton-Raphson iterations give full f32 precision
    let mut i = 0;
    while i < 6
    {
        guess = 0.5 * (guess + (x / guess));
        i += 1;
    }
    guess
}