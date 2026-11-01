# Spend near or past the cost ceiling (ADR 0017) mails the operator. The
# account also runs other workloads, so the budget counts only the Project tag
# the provider applies (ADR 0071). It reads zero until that tag is activated as
# a cost allocation tag, which infra/README.md lists as a step after the first
# apply.
resource "aws_budgets_budget" "monthly" {
  name         = "${local.name}-monthly"
  budget_type  = "COST"
  time_unit    = "MONTHLY"
  limit_amount = "25"
  limit_unit   = "USD"

  cost_filter {
    name   = "TagKeyValue"
    values = ["user:Project$formula-none"]
  }

  notification {
    notification_type          = "FORECASTED"
    comparison_operator        = "GREATER_THAN"
    threshold                  = 100
    threshold_type             = "PERCENTAGE"
    subscriber_email_addresses = [var.billing_alert_email]
  }

  # AWS forecasts only after some spend history, so in the budget's first weeks
  # this is the one warning before the ceiling.
  notification {
    notification_type          = "ACTUAL"
    comparison_operator        = "GREATER_THAN"
    threshold                  = 80
    threshold_type             = "PERCENTAGE"
    subscriber_email_addresses = [var.billing_alert_email]
  }

  notification {
    notification_type          = "ACTUAL"
    comparison_operator        = "GREATER_THAN"
    threshold                  = 100
    threshold_type             = "PERCENTAGE"
    subscriber_email_addresses = [var.billing_alert_email]
  }
}
