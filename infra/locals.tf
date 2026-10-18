locals {
  name = "formula-none"

  # Where the Compose file lives on the instance; first boot creates it and
  # the deploy document runs in it.
  work_dir = "/opt/formula-none"
}
