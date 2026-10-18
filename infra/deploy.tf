# The one Run Command the deploy role may invoke: pull the image tagged with
# the merged commit and restart the Compose stack. Keeping the script here,
# rather than granting AWS-RunShellScript, means CI cannot run anything else
# on the instance.
resource "aws_ssm_document" "deploy" {
  name            = "${local.name}-deploy"
  document_type   = "Command"
  document_format = "YAML"

  content = templatefile("${path.module}/deploy.yaml.tftpl", {
    region         = var.region
    registry       = split("/", aws_ecr_repository.server.repository_url)[0]
    repository_url = aws_ecr_repository.server.repository_url
  })
}
