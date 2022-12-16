resource "aws_key_pair" "deployer_key" {
  key_name = "vnderlev@DESKTOP-FIQR0CK"
  public_key = file("~/.ssh/id_rsa.pub")
}
