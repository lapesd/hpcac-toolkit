# Generated by Django 4.2.1 on 2023-05-30 08:55

from django.db import migrations, models


class Migration(migrations.Migration):
    dependencies = [
        ("clusters", "0001_initial"),
    ]

    operations = [
        migrations.AddField(
            model_name="clusterconfiguration",
            name="vm_username",
            field=models.TextField(default="root"),
        ),
    ]