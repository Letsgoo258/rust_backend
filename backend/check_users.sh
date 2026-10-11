#!/bin/bash
source .env
curl -X POST -H "Content-Type: application/json" -d '{"school_name": "Eravaya HQ", "school_code": "HQ01", "admin_username": "kabhiram67", "admin_email": "kabhiram67@gmail.com", "admin_password": "password123", "first_name": "K", "last_name": "Abhiram"}' http://127.0.0.1:8089/api/v1/auth/setup
