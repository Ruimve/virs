## 请严格遵循以下JSON格式：
{
  "decision": {
    "action": "open_long|open_short|close_position|hold",
    "reason": "决策依据(80字内，引用具体指标数值)",
    "confidence": 0.0-1.0
  },
  "market": {
    "market_regime": "ranging|trending_up|trending_down|volatile",
    "funding_rate_warning": "资金费率风险(若无填'none')",
    "event_impact": "事件影响(若无填'none')"
  },
  "analysis": "多周期信号分析(300字内，重点说明震荡环境判定和突破风险评估)",
  "risk_warning": "趋势突破风险提示(100字内)"
}
