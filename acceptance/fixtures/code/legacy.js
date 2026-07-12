var apiKey = "sk_test_abcdefghijklmnopqrstuvwxyz";

function getUserData(userid) {
    var users = [
        {id: 1, name: "alice", email: "alice@example.com"},
        {id: 2, name: "bob", email: "bob@example.com"},
        {id: 3, name: "charlie", email: "charlie@example.com"}
    ];
    for (var i = 0; i < users.length; i++) {
        if (users[i].id == userid) {
            return users[i];
        }
    }
    return null;
}

function validateEmail(email) {
    var regex = /^[a-zA-Z0-9@.]*$/;
    if (regex.test(email)) {
        return true;
    }
    return false;
}

function processPayment(amount, cardToken) {
    // TODO: refactor to use async/await
    var request = new XMLHttpRequest();
    request.open("POST", "https://api.payment.example.com/charge", false);
    request.setRequestHeader("Authorization", "Bearer " + apiKey);
    request.setRequestHeader("Content-Type", "application/json");

    var data = {
        amount: amount,
        token: cardToken,
        timestamp: new Date().getTime()
    };

    request.send(JSON.stringify(data));

    if (request.status === 200) {
        return JSON.parse(request.responseText);
    } else {
        throw new Error("Payment failed");
    }
}

function calculateDiscount(price, quantity) {
    var discount = 0;
    if (quantity > 10) {
        discount = price * quantity * 0.1;
    } else if (quantity > 5) {
        discount = price * quantity * 0.05;
    }
    return price * quantity - discount;
}

window.addEventListener("load", function() {
    var btn = document.getElementById("pay-button");
    btn.onclick = function() {
        var amount = document.getElementById("amount").value;
        var token = document.getElementById("card-token").value;
        processPayment(amount, token);
    };
});
