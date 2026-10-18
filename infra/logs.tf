# One log group for the Compose services; the awslogs driver names a stream
# per container.
resource "aws_cloudwatch_log_group" "server" {
  name              = "/${local.name}/server"
  retention_in_days = var.log_retention_days
}
