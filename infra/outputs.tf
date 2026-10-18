output "instance_id" {
  description = "Target for `aws ssm start-session --target`."
  value       = aws_instance.server.id
}

output "elastic_ip" {
  value = aws_eip.server.public_ip
}

output "hostname" {
  value = aws_route53_record.game.fqdn
}

output "ecr_repository_url" {
  value = aws_ecr_repository.server.repository_url
}

output "github_actions_role_arn" {
  description = "Value for the deploy workflow's role-to-assume."
  value       = aws_iam_role.github_actions.arn
}

output "deploy_document_name" {
  value = aws_ssm_document.deploy.name
}

output "log_group_name" {
  value = aws_cloudwatch_log_group.server.name
}
