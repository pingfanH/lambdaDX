//! Exact accuracy formulas from LnmaiCore.Score.computeAccRates.
use crate::{natural::Natural, rat::Rat};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccRates {
    pub classic_acc_plus: Rat,
    pub classic_acc_minus: Rat,
    pub dx_acc_minus101: Rat,
    pub dx_acc_minus100: Rat,
    pub dx_acc_plus: Rat,
}

pub fn compute_acc_rates(
    total_base: impl Into<Natural>,
    current_base: impl Into<Natural>,
    lost_base: impl Into<Natural>,
    total_extra: impl Into<Natural>,
    current_extra: impl Into<Natural>,
    lost_extra: impl Into<Natural>,
    current_extra_classic: impl Into<Natural>,
) -> AccRates {
    let total_base = total_base.into();
    let current_base = current_base.into();
    let lost_base = lost_base.into();
    let total_extra = total_extra.into();
    let current_extra = current_extra.into();
    let lost_extra = lost_extra.into();
    let current_extra_classic = current_extra_classic.into();
    let tb = Rat::from_int(&total_base);
    let cb = Rat::from_int(current_base);
    let earned_base = tb.sub(Rat::from_int(lost_base));
    let te = Rat::from_int(total_extra.clone().max(1.into()));
    let ce = Rat::from_int(current_extra);
    let earned_extra = Rat::from_int(total_extra).sub(Rat::from_int(lost_extra));
    let cc = Rat::from_int(current_extra_classic);
    let hundred = Rat::from_int(100);
    if total_base == 0 {
        return AccRates {
            classic_acc_plus: Rat::zero(),
            classic_acc_minus: Rat::zero(),
            dx_acc_minus101: Rat::zero(),
            dx_acc_minus100: Rat::zero(),
            dx_acc_plus: Rat::zero(),
        };
    }
    AccRates {
        classic_acc_plus: cb.add(&cc).div(&tb).mul(&hundred),
        classic_acc_minus: earned_base.add(&cc).div(&tb).mul(&hundred),
        dx_acc_minus101: earned_base
            .div(&tb)
            .add(earned_extra.div(te.mul(&hundred)))
            .mul(&hundred),
        dx_acc_minus100: earned_base
            .div(&tb)
            .add(ce.div(te.mul(&hundred)))
            .mul(&hundred),
        dx_acc_plus: cb.div(&tb).add(ce.div(te.mul(&hundred))).mul(&hundred),
    }
}
