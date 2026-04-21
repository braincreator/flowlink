# FlowLink — Yandex Cloud Terraform Configuration
# Deploys relay to Yandex Compute Cloud with managed PostgreSQL

terraform {
  required_providers {
    yandex = {
      source  = "yandex-cloud/yandex"
      version = "~> 0.100"
    }
  }

  backend "s3" {
    endpoint   = "storage.yandexcloud.net"
    bucket     = "flowlink-terraform-state"
    region     = "ru-central1"
    key        = "terraform.tfstate"
    skip_region_validation      = true
    skip_credentials_validation = true
  }
}

provider "yandex" {
  zone = "ru-central1-a"
}

# Variables
variable "yc_token" { sensitive = true }
variable "yc_cloud_id" {}
variable "yc_folder_id" {}
variable "relay_db_password" { sensitive = true }
variable "jwt_secret" { sensitive = true }
variable "tg_bot_token" { sensitive = true }

# Network
resource "yandex_vpc_network" "flowlink" {
  name = "flowlink-network"
}

resource "yandex_vpc_subnet" "flowlink" {
  name           = "flowlink-subnet"
  zone           = "ru-central1-a"
  network_id     = yandex_vpc_network.flowlink.id
  v4_cidr_blocks = ["10.128.0.0/24"]
}

resource "yandex_vpc_security_group" "flowlink" {
  name        = "flowlink-sg"
  network_id  = yandex_vpc_network.flowlink.id

  ingress {
    protocol    = "TCP"
    port        = 9081
    description = "Relay HTTP API"
  }

  ingress {
    protocol    = "TCP"
    port        = 9080
    description = "Relay WSS (TLS)"
  }

  ingress {
    protocol    = "TCP"
    port        = 22
    description = "SSH"
  }

  egress {
    protocol    = "ANY"
    description = "Allow all outbound"
  }
}

# Managed PostgreSQL
resource "yandex_mdb_postgresql_cluster" "flowlink" {
  name        = "flowlink-pg"
  environment = "PRODUCTION"
  network_id  = yandex_vpc_network.flowlink.id

  config {
    version = 16
    resources {
      resource_preset_id = "s2.micro"
      disk_type_id       = "network-ssd"
      disk_size          = 20
    }
    postgresql_config = {
      max_connections = 100
    }
  }

  host {
    zone             = "ru-central1-a"
    subnet_id        = yandex_vpc_subnet.flowlink.id
    assign_public_ip = false
  }
}

resource "yandex_mdb_postgresql_database" "flowlink" {
  cluster_id = yandex_mdb_postgresql_cluster.flowlink.id
  name       = "flowlink"
  owner      = "flowlink"
}

resource "yandex_mdb_postgresql_user" "flowlink" {
  cluster_id = yandex_mdb_postgresql_cluster.flowlink.id
  name       = "flowlink"
  password   = var.relay_db_password
}

# Compute Instance
data "yandex_compute_image" "ubuntu" {
  family = "ubuntu-2204-lts"
}

resource "yandex_compute_instance" "relay" {
  name     = "flowlink-relay"
  zone     = "ru-central1-a"

  resources {
    cores  = 4
    memory = 8
  }

  boot_disk {
    initialize_params {
      image_id = data.yandex_compute_image.ubuntu.id
      size     = 40
    }
  }

  network_interface {
    subnet_id          = yandex_vpc_subnet.flowlink.id
    security_group_ids = [yandex_vpc_security_group.flowlink.id]
    nat                = true
  }

  metadata = {
    ssh-keys = "flowlink:${file("~/.ssh/id_ed25519.pub")}"
  }

  connection {
    type        = "ssh"
    user        = "flowlink"
    host        = self.network_interface[0].nat_ip_address
    private_key = file("~/.ssh/id_ed25519")
  }

  provisioner "remote-exec" {
    inline = [
      "sudo apt-get update",
      "sudo apt-get install -y ca-certificates curl",
      "sudo mkdir -p /opt/flowlink/bin /etc/flowlink",
    ]
  }
}

# Relay config
resource "local_file" "relay_config" {
  content = jsonencode({
    http_addr   = "0.0.0.0:9081"
    wss_addr    = "0.0.0.0:9080"
    tg_bot_token = var.tg_bot_token
    database = {
      primary = "postgresql://flowlink:${var.relay_db_password}@${yandex_mdb_postgresql_cluster.flowlink.host[0].fqdn}:6432/flowlink"
      pool_size = 10
      migrate_on_start = true
    }
    auth = {
      jwt_secret = var.jwt_secret
    }
  })
  filename = "${path.module}/relay.json"
}

output "relay_ip" {
  value = yandex_compute_instance.relay.network_interface[0].nat_ip_address
}

output "db_host" {
  value = yandex_mdb_postgresql_cluster.flowlink.host[0].fqdn
}
