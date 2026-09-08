# Concept

Application d'envoi de messages par bluetooth dans un réseau connecté sans connexion internet.

# Besoin métier

- Envoyer un message
- Recevoir un message
- Sécuriser l'envoi des messages
- Sécuriser le contenu des messages
- Pas en connection directe
- Pas de connexion internet
- Pas de connexion mobile
- Être déconnecté ne doit pas être bloquant
- Le message se diffuse sur le réseau
- Suivre les messages (leur transit et status) sur un dashboard si connection wifi (non bloquant)
- Passer par des téléphones mobiles (android et ios) et par des cartes arduino avec bluetooth et wifi (FREENOVE ESP32 WROOM)

# Status des messages

1. En attente
2. Parti
3. Distribué
4. Lu/vu