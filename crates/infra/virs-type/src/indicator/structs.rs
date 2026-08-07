use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::enums::{IndicatorSpec, IndicatorValue};


/* 指标集：以 HashMap 存储指标规格与值的映射，提供查询、插入和类型化访问方法。
 * 计算逻辑由 virs-indicator crate 负责，本结构仅作为数据容器。 */
#[derive(Debug, Clone, Default)]
pub struct IndicatorSet {
    values: HashMap<IndicatorSpec, IndicatorValue>,
}

impl Serialize for IndicatorSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let pairs: Vec<(&IndicatorSpec, &IndicatorValue)> = self.values.iter().collect();
        pairs.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for IndicatorSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let pairs: Vec<(IndicatorSpec, IndicatorValue)> = Vec::deserialize(deserializer)?;
        let values = pairs.into_iter().collect();
        Ok(Self { values })
    }
}

impl IndicatorSet {

    pub fn new() -> Self {
        Self::default()
    }


    pub fn with_value(spec: IndicatorSpec, value: IndicatorValue) -> Self {
        let mut set = Self::default();
        set.values.insert(spec, value);
        set
    }


    pub fn insert(&mut self, spec: IndicatorSpec, value: IndicatorValue) -> &mut Self {
        self.values.insert(spec, value);
        self
    }


    pub fn get(&self, spec: &IndicatorSpec) -> Option<&IndicatorValue> {
        self.values.get(spec)
    }


    pub fn get_num(&self, spec: &IndicatorSpec) -> Option<f64> {
        match self.values.get(spec)? {
            IndicatorValue::Num(v) => Some(*v),
            _ => None,
        }
    }


    pub fn get_int(&self, spec: &IndicatorSpec) -> Option<i32> {
        match self.values.get(spec)? {
            IndicatorValue::Int(v) => Some(*v),
            _ => None,
        }
    }


    pub fn get_str(&self, spec: &IndicatorSpec) -> Option<&str> {
        match self.values.get(spec)? {
            IndicatorValue::Str(v) => Some(v.as_str()),
            _ => None,
        }
    }
}
