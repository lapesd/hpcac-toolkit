# Generated by Django 4.2.1 on 2023-12-21 11:02

from django.db import migrations, models


class Migration(migrations.Migration):
    dependencies = [
        ("clusters", "0008_clusterconfiguration_vcpus"),
    ]

    operations = [
        migrations.AddField(
            model_name="clusterconfiguration",
            name="spawn_time",
            field=models.IntegerField(default=0),
        ),
    ]